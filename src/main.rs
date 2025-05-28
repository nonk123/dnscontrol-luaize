use std::{
    fs::File,
    io::{BufRead, Cursor, Write},
    path::{Path, PathBuf},
    process::Command,
};

use color_eyre::eyre::eyre;

type Result<T> = color_eyre::eyre::Result<T>;

fn main() -> Result<()> {
    let _ = color_eyre::install();

    let mut out = File::create(js_path()?)?;
    luaize(&lua_path()?, &mut out)?;

    let mut cmd = Command::new("dnscontrol");
    for arg in std::env::args() {
        cmd.arg(arg);
    }

    let mut child = cmd.spawn()?;
    child.wait()?;
    Ok(())
}

fn lua_path() -> Result<PathBuf> {
    Ok(std::env::current_dir()?.join("dnscontrol.lua"))
}

fn js_path() -> Result<PathBuf> {
    Ok(std::env::current_dir()?.join("dnscontrol.js"))
}

fn funcall_to_string(expr: &lua_parser::ExprFunctionCall) -> Result<String> {
    let mut s = String::new();

    if let Some(ref method) = expr.method {
        s += &method;
    } else {
        s += &expr_to_str(&expr.prefix)?;
    }

    s += "(";
    if expr.method.is_some() {
        s += &expr_to_str(&expr.prefix)?;
        if !expr.args.args.is_empty() {
            s += ", ";
        }
    }

    let mut iter = expr.args.args.iter().peekable();
    while let Some(expr) = iter.next() {
        s += &expr_to_str(expr)?;
        if iter.peek().is_some() {
            s += ", ";
        }
    }

    s += ")";

    Ok(s)
}

fn expr_to_str(expr: &lua_parser::Expression) -> Result<String> {
    let mut s = String::new();

    use lua_parser::{ExprBinary, ExprUnary, Expression, TableField};
    match expr {
        Expression::Ident(expr) => {
            if expr.name.as_str() == "this" {
                return Err(eyre!("Illegal identifier 'this'"));
            } else {
                s += &expr.name.to_string();
            }
        }
        Expression::Bool(expr) => {
            s = String::from(if expr.value { "true" } else { "false" });
        }
        Expression::Numeric(expr) => {
            s = expr.value.to_string();
        }
        Expression::Nil(_) => {
            s = String::from("undefined");
        }
        Expression::String(expr) => {
            s += "\""; // TODO: UTF-8?
            for c in &expr.value {
                s += &format!("\\x{:x?}", c);
            }
            s += "\"";
        }
        Expression::Unary(expr) => match expr {
            ExprUnary::Length(expr) => {
                s += "(";
                s += &expr_to_str(&expr.value)?;
                s += ".length)";
            }
            ExprUnary::Minus(expr) => {
                s += "(-";
                s += &expr_to_str(&expr.value)?;
                s += ")"
            }
            ExprUnary::Plus(expr) => {
                s += "(+";
                s += &expr_to_str(&expr.value)?;
                s += ")"
            }
            other => {
                return Err(eyre!("Expression currently unsupported: {:?}", other));
            }
        },
        Expression::Binary(expr) => {
            let (lhs, rhs, op) = match expr.clone() {
                ExprBinary::Add(expr) => (expr.lhs, expr.rhs, "+"),
                ExprBinary::Sub(expr) => (expr.lhs, expr.rhs, "-"),
                ExprBinary::Mul(expr) => (expr.lhs, expr.rhs, "*"),
                ExprBinary::Div(expr) => (expr.lhs, expr.rhs, "/"),
                ExprBinary::FloorDiv(expr) => (expr.lhs, expr.rhs, "/"), // TODO: differentiate from regular division...
                ExprBinary::Mod(expr) => (expr.lhs, expr.rhs, "*"),
                ExprBinary::Concat(expr) => (expr.lhs, expr.rhs, "+"),
                ExprBinary::Equal(expr) => (expr.lhs, expr.rhs, "==="),
                ExprBinary::NotEqual(expr) => (expr.lhs, expr.rhs, "!=="),
                ExprBinary::GreaterThan(expr) => (expr.lhs, expr.rhs, ">"),
                ExprBinary::GreaterEqual(expr) => (expr.lhs, expr.rhs, ">="),
                ExprBinary::LessThan(expr) => (expr.lhs, expr.rhs, "<"),
                ExprBinary::LessEqual(expr) => (expr.lhs, expr.rhs, "<="),
                other => return Err(eyre!("Unsupported binary operator: {:?}", other)),
            };

            s += "(";
            s += &expr_to_str(&lhs)?;
            s += op;
            s += &expr_to_str(&rhs)?;
            s += ")";
        }
        Expression::FunctionCall(expr) => {
            s = funcall_to_string(expr)?;
        }
        Expression::Table(expr) => {
            s += "({";

            let mut iter = expr.fields.iter().peekable();
            while let Some(field) = iter.next() {
                match field {
                    TableField::KeyValue(kv) => {
                        s += &expr_to_str(&kv.key)?;
                        s += ": ";
                        s += &expr_to_str(&kv.value)?;
                    }
                    TableField::NameValue(nv) => {
                        s += "\"";
                        s += &nv.name;
                        s += "\": ";
                        s += &expr_to_str(&nv.value)?;
                    }
                    TableField::Value(_) => {
                        return Err(eyre!("Table value without a key currently unsupported"));
                    }
                };

                if iter.peek().is_some() {
                    s += ", ";
                }
            }

            s += "})";
        }
        Expression::TableIndex(expr) => {
            s += "(";
            s += &expr_to_str(&expr.table)?;
            s += "[";
            s += &expr_to_str(&expr.index)?;
            s += "])";
        }
        other => return Err(eyre!("Expression unsupported: {:?}", other)),
    }

    Ok(s)
}

struct BlockWriter {
    indent: usize,
}

impl BlockWriter {
    fn new() -> Self {
        Self { indent: 0 }
    }

    fn write_block(&mut self, out: &mut dyn Write, block: &lua_parser::Block) -> Result<()> {
        use lua_parser::Statement::*;

        let real_out = out;

        let mut buf = Vec::new();
        let out = &mut Cursor::new(&mut buf);

        self.indent += 1;

        for stmt in &block.statements {
            match stmt {
                None(_) => {
                    writeln!(out, ";")?;
                }
                Assignment(stmt) => {
                    if stmt.lhs.len() > 1 || stmt.rhs.len() > 1 {
                        return Err(eyre!("Parallel assignment currently unsupported"));
                    }

                    writeln!(
                        out,
                        "{} = {};",
                        expr_to_str(&stmt.lhs[0])?,
                        expr_to_str(&stmt.rhs[0])?
                    )?;
                }
                LocalDeclaration(stmt) => {
                    if stmt.names.len() > 1 {
                        return Err(eyre!("Local multiple declarations currently unsupported"));
                    }
                    let Some(values) = &stmt.values else {
                        return Err(eyre!("Local declarations without assignment unsupported"));
                    };
                    writeln!(
                        out,
                        "var {} = {};",
                        stmt.names[0].name,
                        expr_to_str(&values[0])?
                    )?;
                }
                If(stmt) => {
                    writeln!(out, "if ({}) {{", expr_to_str(&stmt.condition)?)?;
                    self.write_block(out, &stmt.block)?;
                    writeln!(out, "}}")?;

                    for stmt in &stmt.else_ifs {
                        writeln!(out, "else if ({}) {{", expr_to_str(&stmt.condition)?)?;
                        self.write_block(out, &stmt.block)?;
                        writeln!(out, "}}")?;
                    }

                    if let Some(block) = &stmt.else_block {
                        writeln!(out, "else {{")?;
                        self.write_block(out, &block)?;
                        writeln!(out, "}}")?;
                    }
                }
                While(stmt) => {
                    writeln!(out, "while ({}) {{", expr_to_str(&stmt.condition)?)?;
                    self.write_block(out, &stmt.block)?;
                    writeln!(out, "}}")?;
                }
                For(stmt) => {
                    writeln!(
                        out,
                        "for (var {} = {}; {}; {}) {{",
                        stmt.name,
                        expr_to_str(&stmt.start)?,
                        expr_to_str(&stmt.end)?,
                        expr_to_str(&stmt.step)?
                    )?;
                    self.write_block(out, &stmt.block)?;
                    writeln!(out, "}}")?;
                }
                Break(_) => {
                    writeln!(out, "break;")?;
                }
                Do(stmt) => {
                    writeln!(out, "(function() {{")?;
                    self.write_block(out, &stmt.block)?;
                    writeln!(out, "}})();")?;
                }
                FunctionCall(expr) => {
                    writeln!(out, "{};", &funcall_to_string(expr)?)?;
                }
                FunctionDefinition(stmt) => {
                    if stmt.body.parameters.variadic {
                        return Err(eyre!("Variadic functions currently unsupported"));
                    }
                    writeln!(
                        out,
                        "function {}({}) {{",
                        stmt.name.names[0],
                        stmt.body
                            .parameters
                            .names
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(", ")
                    )?;
                    self.write_block(out, &stmt.body.block)?;
                    writeln!(out, "}}")?;
                }
                FunctionDefinitionLocal(stmt) => {
                    if stmt.body.parameters.variadic {
                        return Err(eyre!("Variadic functions currently unsupported"));
                    }
                    writeln!(
                        out,
                        "var {} = ({}) => {{",
                        stmt.name,
                        stmt.body
                            .parameters
                            .names
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(", ")
                    )?;
                    self.write_block(out, &stmt.body.block)?;
                    writeln!(out, "}};")?;
                }
                other => {
                    return Err(eyre!("Statement unsupported: {:?}", other));
                }
            }
        }

        if let Some(stmt) = &block.return_statement {
            if stmt.values.len() > 1 {
                return Err(eyre!("Multiple return values currently unsupported"));
            }
            writeln!(out, "return {};", expr_to_str(&stmt.values[0])?)?;
        }

        self.indent -= 1;

        for line in buf.lines() {
            let line = line?;
            let indent = "    ".repeat(self.indent);
            writeln!(real_out, "{}{}", indent, line)?;
        }

        Ok(())
    }
}

pub fn luaize(path: &Path, out: &mut dyn Write) -> Result<()> {
    let source = std::fs::read(path)?;
    let ast = lua_parser::parse_bytes(&source)?;
    return BlockWriter::new().write_block(out, &ast);
}

#[cfg(test)]
mod tests {
    use std::{fs::File, path::PathBuf};

    fn test_path(name: &str) -> PathBuf {
        std::env::current_dir()
            .unwrap()
            .join("tests")
            .join(name)
            .with_extension("lua")
    }

    fn run_test(name: &str) {
        let _ = color_eyre::install();

        let inpath = test_path(name);
        println!("Testing {:?}", inpath);

        let outpath = std::env::temp_dir().join(name).with_extension("js");
        let mut out = File::create(outpath).unwrap();

        super::luaize(&inpath, &mut out).expect("Failed to luaize");
    }

    macro_rules! test {
        ($name:ident) => {
            #[test]
            fn $name() {
                run_test(stringify!($name));
            }
        };
    }

    test!(basic);
    test!(colon);
}
