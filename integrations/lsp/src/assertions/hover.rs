use common::concat_str;
use lsp::features::find_hover;
use tower_lsp::lsp_types::{Hover, HoverContents, MarkedString, Position};

use crate::config::HoverTest;
use crate::environment::WorkspaceEnvironment;

fn extract_hover_text(hover: &Hover) -> String {
    match &hover.contents {
        HoverContents::Scalar(MarkedString::String(text)) => text.clone(),
        HoverContents::Scalar(MarkedString::LanguageString(language_string)) => {
            language_string.value.clone()
        }
        HoverContents::Array(elements) => {
            let mut combined = String::new();
            for element in elements {
                match element {
                    MarkedString::String(text) => {
                        combined.push_str(text);
                        combined.push('\n');
                    }
                    MarkedString::LanguageString(language_string) => {
                        combined.push_str(&language_string.value);
                        combined.push('\n');
                    }
                }
            }
            combined
        }
        HoverContents::Markup(markup) => markup.value.clone(),
    }
}

pub fn evaluate_hover_test(
    environment: &WorkspaceEnvironment,
    test_case: &HoverTest,
) -> Result<(), String> {
    let file_url = environment.resolve_file_url(&test_case.file)?;
    let source_code = environment
        .get_file_source(&file_url)
        .ok_or_else(|| concat_str!("Source file not found in index: ", &test_case.file))?;

    let cursor_position = Position::new(test_case.position.line, test_case.position.col);
    let hover_result = find_hover(
        &environment.index,
        &file_url,
        &test_case.symbol,
        cursor_position,
        &source_code,
    );

    let Some(hover) = hover_result else {
        return Err(concat_str!(
            "No hover information found for '",
            &test_case.symbol,
            "' at line ",
            &test_case.position.line.to_string(),
            ", col ",
            &test_case.position.col.to_string(),
            " in ",
            &test_case.file
        ));
    };

    let hover_text = extract_hover_text(&hover);

    for needle in &test_case.contains {
        if !hover_text.contains(needle) {
            return Err(concat_str!(
                "Hover content missing expected substring: '",
                needle,
                "'. Full hover text was:\n",
                &hover_text
            ));
        }
    }

    for needle in &test_case.not_contains {
        if hover_text.contains(needle) {
            return Err(concat_str!(
                "Hover content contained forbidden substring: '",
                needle,
                "'. Full hover text was:\n",
                &hover_text
            ));
        }
    }

    Ok(())
}
