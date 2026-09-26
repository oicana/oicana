use oicana_template::PdfStandard;
use oicana_world::diagnostics::TemplateDiagnostics;
use thiserror::Error;
use typst::{foundations::Smart, layout::PageRanges};
use typst_layout::PagedDocument;
use typst_pdf::{PdfOptions, PdfStandards};

use crate::pages::PageRange;

fn to_typst_standard(standard: oicana_template::PdfStandard) -> typst_pdf::PdfStandard {
    match standard {
        oicana_template::PdfStandard::V_1_4 => typst_pdf::PdfStandard::V_1_4,
        oicana_template::PdfStandard::V_1_5 => typst_pdf::PdfStandard::V_1_5,
        oicana_template::PdfStandard::V_1_6 => typst_pdf::PdfStandard::V_1_6,
        oicana_template::PdfStandard::V_1_7 => typst_pdf::PdfStandard::V_1_7,
        oicana_template::PdfStandard::V_2_0 => typst_pdf::PdfStandard::V_2_0,
        oicana_template::PdfStandard::A_1b => typst_pdf::PdfStandard::A_1b,
        oicana_template::PdfStandard::A_1a => typst_pdf::PdfStandard::A_1a,
        oicana_template::PdfStandard::A_2b => typst_pdf::PdfStandard::A_2b,
        oicana_template::PdfStandard::A_2u => typst_pdf::PdfStandard::A_2u,
        oicana_template::PdfStandard::A_2a => typst_pdf::PdfStandard::A_2a,
        oicana_template::PdfStandard::A_3b => typst_pdf::PdfStandard::A_3b,
        oicana_template::PdfStandard::A_3u => typst_pdf::PdfStandard::A_3u,
        oicana_template::PdfStandard::A_3a => typst_pdf::PdfStandard::A_3a,
        oicana_template::PdfStandard::A_4 => typst_pdf::PdfStandard::A_4,
        oicana_template::PdfStandard::A_4f => typst_pdf::PdfStandard::A_4f,
        oicana_template::PdfStandard::A_4e => typst_pdf::PdfStandard::A_4e,
        oicana_template::PdfStandard::Ua_1 => typst_pdf::PdfStandard::Ua_1,
    }
}

/// Whether the standard is only met by a tagged PDF.
///
/// Mirrors krilla's `requires_tagging`.
fn requires_tagging(standard: oicana_template::PdfStandard) -> bool {
    use oicana_template::PdfStandard::*;
    match standard {
        A_1a | A_2a | A_3a | Ua_1 => true,
        V_1_4 | V_1_5 | V_1_6 | V_1_7 | V_2_0 | A_1b | A_2b | A_2u | A_3b | A_3u | A_4 | A_4f
        | A_4e => false,
    }
}

/// Check whether the given list of Oicana PDF standards forms a combination
/// Typst can produce (at most one base version, at most one PDF/A standard
/// and at most one PDF/UA standard, all sharing overlapping PDF versions).
pub fn validate_pdf_standards(standards: &[oicana_template::PdfStandard]) -> Result<(), String> {
    let typst_standards: Vec<_> = standards.iter().map(|s| to_typst_standard(*s)).collect();
    PdfStandards::new(&typst_standards)
        .map(|_| ())
        .map_err(|e| format!("Invalid combination of PDF standards: {}", e.message()))
}

/// Check that no standard requires a tagged PDF when the template disables tagging.
pub fn validate_pdf_tagging(
    standards: &[oicana_template::PdfStandard],
    tagged: bool,
) -> Result<(), String> {
    match standards.iter().find(|s| !tagged && requires_tagging(**s)) {
        Some(standard) => Err(PdfExportError::TaggingDisabled(*standard).to_string()),
        None => Ok(()),
    }
}

/// An error that occurred while exporting a document to PDF.
#[derive(Debug, Error)]
pub enum PdfExportError {
    /// The requested page range selected none of the document's pages.
    #[error("The requested page range selected no pages of the document")]
    NoPagesSelected,
    /// A standard requires a tagged PDF, but partial exports cannot be tagged.
    #[error(
        "PDF standard {0} requires a tagged PDF, but a page range cannot be tagged. \
         Export the whole document or choose different standards"
    )]
    PageRangeCannotBeTagged(PdfStandard),
    /// A standard requires a tagged PDF, but the template disables tagging.
    #[error("PDF standard {0} requires a tagged PDF, but the template disables tagging")]
    TaggingDisabled(PdfStandard),
    /// The configured PDF standards cannot be combined.
    #[error("Invalid combination of PDF standards: {0}")]
    InvalidStandards(String),
    /// Typst failed to produce the PDF.
    #[error("{0}")]
    ExportFailed(String),
}

/// Export the document to PDF.
///
/// When `pages` is `None` the whole document is exported; otherwise only the
/// pages in the range are exported.
///
/// `tagged` requests an accessible, tagged PDF. Tagging is always disabled when
/// exporting a subset of the pages, because Typst cannot tag partial exports.
/// The export fails if tagging ends up disabled while the standards require it.
pub fn export_pdf<Diagnostics: TemplateDiagnostics>(
    document: &PagedDocument,
    diagnostics: &Diagnostics,
    standards: &[oicana_template::PdfStandard],
    tagged: bool,
    pages: Option<&PageRange>,
) -> Result<Vec<u8>, PdfExportError> {
    let typst_standards: Vec<_> = standards.iter().map(|s| to_typst_standard(*s)).collect();

    let page_count = document.pages().len();
    let selected_count = match pages {
        Some(range) => range.selected_indices(page_count).len(),
        None => page_count,
    };
    if selected_count == 0 {
        return Err(PdfExportError::NoPagesSelected);
    }

    // Typst refuses to produce a tagged PDF when a page range is set. Only
    // pass the range and disable tagging when pages are actually skipped.
    let skips_pages = selected_count != page_count;
    let tagged = tagged && !skips_pages;

    if !tagged {
        if let Some(standard) = standards.iter().find(|s| requires_tagging(**s)) {
            return Err(if skips_pages {
                PdfExportError::PageRangeCannotBeTagged(*standard)
            } else {
                PdfExportError::TaggingDisabled(*standard)
            });
        }
    }

    let options = PdfOptions {
        ident: Smart::Auto,
        creator: Smart::Auto,
        timestamp: None,
        page_ranges: if skips_pages {
            pages.map(PageRanges::from)
        } else {
            None
        },
        tagged,
        standards: PdfStandards::new(&typst_standards)
            .map_err(|e| PdfExportError::InvalidStandards(e.message().to_string()))?,
        pretty: false,
    };

    typst_pdf::pdf(document, &options).map_err(|source_error| {
        PdfExportError::ExportFailed(
            String::from_utf8_lossy(&diagnostics.format_diagnostics(source_error)).into(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use oicana_files::preloaded::PreloadedTemplate;
    use oicana_world::manifest::OicanaWorldFiles;
    use oicana_world::world::OicanaWorld;
    use std::collections::HashMap;

    fn manifest() -> &'static str {
        r#"
        [package]
        name = "test"
        version = "0.1.0"
        entrypoint = "main.typ"
        
        [tool.oicana]
        manifest_version = 1
        "#
    }

    fn simple_template() -> PreloadedTemplate {
        let mut files = HashMap::new();
        files.insert("typst.toml".to_owned(), manifest().to_owned());
        files.insert(
            "main.typ".to_owned(),
            "#set page(width: 200pt, height: 100pt)\n#set document(date: datetime(year: 2020,month: 10,day: 4))\nHello".to_owned(),
        );
        PreloadedTemplate::new(files)
    }

    fn multipage_template() -> PreloadedTemplate {
        let mut files = HashMap::new();
        files.insert("typst.toml".to_owned(), manifest().to_owned());
        files.insert(
            "main.typ".to_owned(),
            "#set page(width: 200pt, height: 100pt)\n#set document(date: datetime(year: 2020,month: 10,day: 4))\nPage 1\n#pagebreak()\nPage 2\n#pagebreak()\nPage 3".to_owned(),
        );
        PreloadedTemplate::new(files)
    }

    fn accessible_template() -> PreloadedTemplate {
        let mut files = HashMap::new();
        files.insert("typst.toml".to_owned(), manifest().to_owned());
        files.insert(
            "main.typ".to_owned(),
            "#set page(width: 200pt, height: 100pt)\n#set document(title: \"Test\", date: datetime(year: 2020,month: 10,day: 4))\n#set text(lang: \"en\")\nPage 1\n#pagebreak()\nPage 2\n#pagebreak()\nPage 3".to_owned(),
        );
        PreloadedTemplate::new(files)
    }

    const ALL_STANDARDS: [oicana_template::PdfStandard; 17] = {
        use oicana_template::PdfStandard::*;
        [
            V_1_4, V_1_5, V_1_6, V_1_7, V_2_0, A_1b, A_1a, A_2b, A_2u, A_2a, A_3b, A_3u, A_3a, A_4,
            A_4f, A_4e, Ua_1,
        ]
    };

    fn compile(template: PreloadedTemplate) -> (PagedDocument, OicanaWorld<PreloadedTemplate>) {
        let manifest = template.manifest().unwrap();
        let mut world = OicanaWorld::new(template, manifest).unwrap();
        let compiled = world.compile().unwrap();
        (compiled.document, world)
    }

    #[test]
    fn exports_simple_document_to_pdf() {
        let (document, world) = compile(simple_template());
        let pdf = export_pdf(
            &document,
            &world,
            &[oicana_template::PdfStandard::A_3b],
            true,
            None,
        )
        .expect("PDF export to work");

        assert_eq!(&pdf[0..4], b"%PDF");
        let end = String::from_utf8_lossy(&pdf[pdf.len() - 10..]);
        assert!(end.contains("%%EOF"));
    }

    #[test]
    fn exports_multipage_document_to_pdf() {
        let (document, world) = compile(multipage_template());
        let pdf = export_pdf(
            &document,
            &world,
            &[oicana_template::PdfStandard::A_3b],
            true,
            None,
        )
        .unwrap();

        assert!(pdf.len() > 500);
        assert_eq!(&pdf[0..4], b"%PDF");
        let end = String::from_utf8_lossy(&pdf[pdf.len() - 10..]);
        assert!(end.contains("%%EOF"));
    }

    #[test]
    fn pdf_export_is_deterministic() {
        let (doc1, world1) = compile(simple_template());
        let (doc2, world2) = compile(simple_template());

        let pdf1 = export_pdf(
            &doc1,
            &world1,
            &[oicana_template::PdfStandard::A_3b],
            true,
            None,
        )
        .unwrap();
        let pdf2 = export_pdf(
            &doc2,
            &world2,
            &[oicana_template::PdfStandard::A_3b],
            true,
            None,
        )
        .unwrap();

        assert_eq!(pdf1, pdf2);
    }

    #[test]
    fn multipage_pdf_is_larger_than_single_page() {
        let (single_doc, single_world) = compile(simple_template());
        let (multi_doc, multi_world) = compile(multipage_template());

        let single_pdf = export_pdf(
            &single_doc,
            &single_world,
            &[oicana_template::PdfStandard::A_3b],
            true,
            None,
        )
        .unwrap();
        let multi_pdf = export_pdf(
            &multi_doc,
            &multi_world,
            &[oicana_template::PdfStandard::A_3b],
            true,
            None,
        )
        .unwrap();

        assert!(multi_pdf.len() > single_pdf.len());
    }

    #[test]
    fn exports_a_page_range() {
        let (document, world) = compile(multipage_template());

        let single = export_pdf(
            &document,
            &world,
            &[oicana_template::PdfStandard::A_3b],
            true,
            Some(&PageRange::single(0)),
        )
        .unwrap();
        let full = export_pdf(
            &document,
            &world,
            &[oicana_template::PdfStandard::A_3b],
            true,
            None,
        )
        .unwrap();

        assert_eq!(&single[0..4], b"%PDF");
        assert!(single.len() < full.len());
    }

    #[test]
    fn out_of_bounds_range_is_rejected() {
        let (document, world) = compile(multipage_template());

        let result = export_pdf(
            &document,
            &world,
            &[oicana_template::PdfStandard::A_3b],
            true,
            Some(&PageRange::single(99)),
        );

        assert!(result.is_err());
    }

    #[test]
    fn exports_with_different_standard() {
        let (document, world) = compile(simple_template());
        let pdf = export_pdf(
            &document,
            &world,
            &[oicana_template::PdfStandard::A_4],
            true,
            None,
        )
        .unwrap();

        assert_eq!(&pdf[0..4], b"%PDF");
        let end = String::from_utf8_lossy(&pdf[pdf.len() - 10..]);
        assert!(end.contains("%%EOF"));
    }

    #[test]
    fn exports_untagged_document_to_pdf() {
        let (document, world) = compile(simple_template());
        let pdf = export_pdf(
            &document,
            &world,
            &[oicana_template::PdfStandard::A_3b],
            false,
            None,
        )
        .expect("untagged PDF export to work");

        assert_eq!(&pdf[0..4], b"%PDF");
        let end = String::from_utf8_lossy(&pdf[pdf.len() - 10..]);
        assert!(end.contains("%%EOF"));
    }

    #[test]
    fn rejects_incompatible_standards() {
        let (document, world) = compile(simple_template());
        let result = export_pdf(
            &document,
            &world,
            &[
                oicana_template::PdfStandard::A_4,
                oicana_template::PdfStandard::Ua_1,
            ],
            true,
            None,
        );

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid combination of PDF standards"));
    }

    #[test]
    fn validate_pdf_standards_accepts_single_validator() {
        assert!(validate_pdf_standards(&[oicana_template::PdfStandard::A_3b]).is_ok());
    }

    #[test]
    fn validate_pdf_standards_accepts_version_plus_validator() {
        assert!(validate_pdf_standards(&[
            oicana_template::PdfStandard::V_2_0,
            oicana_template::PdfStandard::A_4,
        ])
        .is_ok());
    }

    #[test]
    fn validate_pdf_standards_rejects_two_validators() {
        let err = validate_pdf_standards(&[
            oicana_template::PdfStandard::A_4,
            oicana_template::PdfStandard::Ua_1,
        ])
        .unwrap_err();
        assert!(err.contains("Invalid combination of PDF standards"));
    }

    #[test]
    fn validate_pdf_standards_rejects_two_versions() {
        let err = validate_pdf_standards(&[
            oicana_template::PdfStandard::V_1_7,
            oicana_template::PdfStandard::V_2_0,
        ])
        .unwrap_err();
        assert!(err.contains("Invalid combination of PDF standards"));
    }

    #[test]
    fn validate_pdf_tagging_rejects_standards_requiring_tags_when_untagged() {
        for standard in ALL_STANDARDS {
            assert!(validate_pdf_tagging(&[standard], true).is_ok());
            let result = validate_pdf_tagging(&[standard], false);
            if requires_tagging(standard) {
                assert!(result.unwrap_err().contains("disables tagging"));
            } else {
                assert!(result.is_ok());
            }
        }
    }

    #[test]
    fn page_range_with_a_standard_requiring_tags_is_rejected() {
        let (document, world) = compile(accessible_template());

        for standard in ALL_STANDARDS.into_iter().filter(|s| requires_tagging(*s)) {
            let err = export_pdf(
                &document,
                &world,
                &[standard],
                true,
                Some(&PageRange::single(0)),
            )
            .unwrap_err()
            .to_string();
            assert!(err.contains("requires a tagged PDF"), "{standard}: {err}");
            assert!(err.contains("page range"), "{standard}: {err}");
        }
    }

    #[test]
    fn untagged_export_with_a_standard_requiring_tags_is_rejected() {
        let (document, world) = compile(accessible_template());

        for standard in ALL_STANDARDS.into_iter().filter(|s| requires_tagging(*s)) {
            let err = export_pdf(&document, &world, &[standard], false, None)
                .unwrap_err()
                .to_string();
            assert!(
                err.contains("template disables tagging"),
                "{standard}: {err}"
            );
        }
    }

    #[test]
    fn page_range_with_standards_not_requiring_tags_exports() {
        let (document, world) = compile(accessible_template());

        for standard in ALL_STANDARDS.into_iter().filter(|s| !requires_tagging(*s)) {
            let result = export_pdf(
                &document,
                &world,
                &[standard],
                true,
                Some(&PageRange::single(0)),
            );
            assert!(result.is_ok(), "{standard}: {}", result.unwrap_err());
        }
    }

    #[test]
    fn page_range_selecting_every_page_keeps_ua_1() {
        let (document, world) = compile(accessible_template());

        let result = export_pdf(
            &document,
            &world,
            &[oicana_template::PdfStandard::Ua_1],
            true,
            Some(&PageRange::from(0)),
        );
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }
}
