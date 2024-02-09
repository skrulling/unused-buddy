use swc_common::BytePos;
use swc_ecma_ast::{Decl, ExportDecl};
use swc_ecma_parser::{lexer::Lexer, Parser, StringInput, Syntax, TsConfig};
use swc_ecma_visit::{Visit, VisitWith};

struct ExportedFunctionCounter {
    count: usize,
}

impl Visit for ExportedFunctionCounter {
    fn visit_export_decl(&mut self, export_decl: &ExportDecl) {
        if let Decl::Fn(_) = &export_decl.decl {
            self.count += 1;
        }
    }
}

pub fn find_functions(input: &str) -> usize {
    let mut parser = create_parser_for_input(input);

    let mut counter = ExportedFunctionCounter { count: 0 };

    match parser.parse_module() {
        Ok(module) => {
            module.visit_with(&mut counter);
            counter.count
        }
        Err(e) => {
            eprintln!("Error parsing input: {:?}", e);
            0
        }
    }
}

fn create_parser_for_input(input: &str) -> Parser<Lexer<'_>> {
    let lexer = Lexer::new(
        Syntax::Typescript(TsConfig {
            tsx: false,
            decorators: false,
            dts: false,
            no_early_errors: false,
            disallow_ambiguous_jsx_like: false,
        }),
        swc_ecma_ast::EsVersion::Es2022,
        StringInput::new(input, BytePos(0), BytePos(input.len() as u32)),
        None,
    );
    Parser::new_from(lexer)
}

#[cfg(test)]
mod tests {
    use crate::find_functions;

    #[test]
    fn it_finds_exported_functions() {
        // Example TypeScript code as input
        let ts_code = r#"
            export function exportedFunc1() {}
            function nonExportedFunc() {}
            export function exportedFunc2() {}
            // This is a comment: export function commentedExportedFunc() {}
            export function exportedFunc3() {}
        "#;

        // Assuming find_functions counts the number of exported 'function' declarations
        let count = find_functions(ts_code);

        // Expecting 3 exported functions: exportedFunc1, exportedFunc2, exportedFunc3
        assert_eq!(count, 3, "The count of exported functions should be 3.");
    }
}
