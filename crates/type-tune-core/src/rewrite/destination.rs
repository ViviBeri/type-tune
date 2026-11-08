//! Render destination files from planned imports and declarations.

use std::collections::{BTreeMap, BTreeSet};

use crate::parser::ParsedDeclaration;

use super::{
    format::match_line_endings,
    imports::{TypeImportRequirement, merge_type_imports, render_type_imports},
};

#[derive(Default)]
pub(super) struct DestinationPlan {
    imports: BTreeMap<String, BTreeMap<(String, char), TypeImportRequirement>>,
    declarations: BTreeMap<String, Vec<(String, usize, usize, String)>>,
    semicolons: BTreeSet<String>,
}

impl DestinationPlan {
    pub(super) fn add_source(
        &mut self,
        source_path: &str,
        destination_path: &str,
        mut moved: Vec<&ParsedDeclaration>,
        imports: &BTreeMap<(String, char), TypeImportRequirement>,
        use_semicolons: bool,
    ) {
        if use_semicolons {
            self.semicolons.insert(destination_path.to_string());
        }
        moved.sort_by_key(|declaration| declaration.start_byte);
        merge_type_imports(
            self.imports.entry(destination_path.into()).or_default(),
            imports,
        );
        self.declarations
            .entry(destination_path.into())
            .or_default()
            .extend(moved.iter().map(|declaration| {
                (
                    source_path.to_string(),
                    declaration.start_byte,
                    declaration.end_byte,
                    moved_declaration_text(declaration),
                )
            }));
    }

    pub(super) fn render(
        self,
        sources: &BTreeMap<String, String>,
        output: &mut BTreeMap<String, String>,
    ) {
        render_destinations(
            sources,
            self.imports,
            self.declarations,
            self.semicolons,
            output,
        );
    }
}

fn moved_declaration_text(declaration: &ParsedDeclaration) -> String {
    if declaration.leading_comment_text.is_empty() {
        declaration.declaration_text.clone()
    } else {
        format!(
            "{}\n{}",
            declaration.leading_comment_text, declaration.declaration_text
        )
    }
}

fn render_destinations(
    sources: &BTreeMap<String, String>,
    mut destination_imports: BTreeMap<String, BTreeMap<(String, char), TypeImportRequirement>>,
    destination_declarations: BTreeMap<String, Vec<(String, usize, usize, String)>>,
    semicolons: BTreeSet<String>,
    output: &mut BTreeMap<String, String>,
) {
    for (destination_path, mut declarations) in destination_declarations {
        declarations.sort_by(
            |(left_path, left_start, _, _), (right_path, right_start, _, _)| {
                left_path
                    .cmp(right_path)
                    .then_with(|| left_start.cmp(right_start))
            },
        );
        let line_ending_source = &sources[&declarations[0].0];
        let imports = destination_imports
            .remove(&destination_path)
            .unwrap_or_default();
        let mut previous_declaration: Option<(String, usize)> = None;
        let mut rendered_declarations = String::new();
        for (source_path, start, end, declaration) in declarations {
            if let Some((previous_path, previous_end)) = &previous_declaration {
                let separator = if previous_path == &source_path
                    && !sources[&source_path][*previous_end..start].contains('\n')
                {
                    "\n"
                } else {
                    "\n\n"
                };
                rendered_declarations.push_str(separator);
            }
            rendered_declarations.push_str(&declaration);
            previous_declaration = Some((source_path, end));
        }
        let semicolon = if semicolons.contains(&destination_path) {
            ";"
        } else {
            ""
        };
        output.insert(
            destination_path,
            match_line_endings(
                line_ending_source,
                &format!(
                    "{}{}\n",
                    render_type_imports(&imports, semicolon),
                    rendered_declarations
                ),
            ),
        );
    }
}
