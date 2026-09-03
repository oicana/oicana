//! Hosts can change the limits applied when reading a packed template.

use oicana::files::packed::{PackedTemplate, ZipLimits};
use oicana::fonts::FontSource;
use oicana::Template;
use std::fs::read;
use std::io::Cursor;

/// A packed template with ~20 entries and roughly 160 KiB of content.
fn template_file() -> Vec<u8> {
    read("../../../assets/templates/invoice-0.1.0.zip").expect("read test template fixture")
}

#[test]
fn reads_a_template_that_stays_within_the_limits() {
    Template::<PackedTemplate>::init_with_limits(
        Cursor::new(template_file()),
        ZipLimits::default(),
    )
    .expect("the default limits allow the invoice template");
}

#[test]
fn refuses_a_template_with_too_many_entries() {
    let error = Template::<PackedTemplate>::init_with_limits(
        Cursor::new(template_file()),
        ZipLimits {
            max_entries: 5,
            ..ZipLimits::default()
        },
    )
    .err()
    .expect("a template with more entries than allowed must not initialize");

    assert!(
        error.to_string().contains("exceeding the limit of 5"),
        "got: {error}"
    );
}

#[test]
fn refuses_a_template_with_too_much_content() {
    let error = Template::<PackedTemplate>::init_with_limits(
        Cursor::new(template_file()),
        ZipLimits {
            max_total_decompressed_bytes: 1024,
            ..ZipLimits::default()
        },
    )
    .err()
    .expect("a template with more content than allowed must not initialize");

    assert!(
        error.to_string().contains("limit of 1024 bytes"),
        "got: {error}"
    );
}

#[test]
fn applies_the_limits_when_fonts_are_provided() {
    let font = FontSource::from_path("../../../assets/fonts/oicana-test-font.ttf")
        .expect("read the test font");

    Template::<PackedTemplate>::init_with_fonts_and_limits(
        Cursor::new(template_file()),
        std::slice::from_ref(&font),
        ZipLimits::default(),
    )
    .expect("the default limits allow the invoice template");

    let error = Template::<PackedTemplate>::init_with_fonts_and_limits(
        Cursor::new(template_file()),
        &[font],
        ZipLimits {
            max_entries: 5,
            ..ZipLimits::default()
        },
    )
    .err()
    .expect("a template with more entries than allowed must not initialize");

    assert!(
        error.to_string().contains("exceeding the limit of 5"),
        "got: {error}"
    );
}
