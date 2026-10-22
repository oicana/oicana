"""Tests for the Template class."""
import zipfile
from io import BytesIO
from pathlib import Path

import pytest

from oicana import CompilationMode, OicanaError, Template

MINIMAL_MANIFEST = """\
[package]
name = "template-test"
version = "0.1.0"
entrypoint = "main.typ"

[tool.oicana]
manifest_version = 1
"""


def pack_minimal_template(main_typst: str) -> bytes:
    buffer = BytesIO()
    with zipfile.ZipFile(buffer, "w", zipfile.ZIP_STORED) as archive:
        archive.writestr("typst.toml", MINIMAL_MANIFEST)
        archive.writestr("main.typ", main_typst)
    return buffer.getvalue()


def test_import() -> None:
    """Test that the module can be imported."""
    from oicana import BlobInput, ExportFormat

    assert Template is not None
    assert CompilationMode is not None
    assert BlobInput is not None
    assert ExportFormat is not None


def test_compilation_mode() -> None:
    """Test CompilationMode enum."""
    assert CompilationMode.PRODUCTION.value == "production"
    assert CompilationMode.DEVELOPMENT.value == "development"


def test_export_surfaces_warnings() -> None:
    template = pack_minimal_template(
        '#set text(font: "NonexistentFontTemplate")\nContent'
    )
    with Template(template) as tmpl:
        assert len(tmpl.warnings) == 1

        svg = tmpl.export(export={"format": "svg"})

        assert b"<svg" in svg
        assert len(tmpl.warnings) == 1
        assert "nonexistentfonttemplate" in tmpl.warnings[0].message

        with tmpl.compile() as document:
            assert document.warnings == tmpl.warnings


def test_export_without_warnings_has_none() -> None:
    with Template(pack_minimal_template("Content")) as tmpl:
        tmpl.export(export={"format": "svg"})

        assert tmpl.warnings == []


def future_manifest_template() -> bytes:
    """A template packed by a newer Oicana than this release understands."""
    path = (
        Path(__file__).parents[4]
        / "assets"
        / "templates"
        / "future-manifest-0.1.0.zip"
    )
    return path.read_bytes()


def test_refuses_a_template_packed_by_a_newer_oicana() -> None:
    """Registering a template with an unsupported manifest version fails."""
    with pytest.raises(OicanaError, match="manifest_version 99"):
        Template(future_manifest_template())


def test_oicana_errors_are_runtime_errors() -> None:
    """Callers catching RuntimeError keep catching Oicana failures."""
    assert issubclass(OicanaError, RuntimeError)


INVALID_MODES = ["production", "development", None, 0, 1, "bogus"]


@pytest.mark.parametrize("mode", INVALID_MODES)
def test_registration_rejects_invalid_mode(mode: object) -> None:
    with pytest.raises(TypeError, match="mode must be a CompilationMode"):
        Template(pack_minimal_template("Content"), mode=mode)  # type: ignore[arg-type]


@pytest.mark.parametrize("mode", INVALID_MODES)
def test_export_rejects_invalid_mode(mode: object) -> None:
    """Anything but the enum must not fall back to development mode."""
    with Template(pack_minimal_template("Content")) as tmpl:
        with pytest.raises(TypeError, match="mode must be a CompilationMode"):
            tmpl.export_pdf(mode=mode)  # type: ignore[arg-type]
        with pytest.raises(TypeError, match="mode must be a CompilationMode"):
            tmpl.compile(mode=mode)  # type: ignore[arg-type]


@pytest.mark.parametrize("mode", INVALID_MODES)
def test_export_once_rejects_invalid_mode(mode: object) -> None:
    with pytest.raises(TypeError, match="mode must be a CompilationMode"):
        Template.export_once(pack_minimal_template("Content"), mode=mode)  # type: ignore[arg-type]
