"""E2E tests for Oicana Python integration."""

import threading
import time
from pathlib import Path

import pytest

from oicana import (
    BlobInput,
    BlobInputDefinition,
    CompilationMode,
    JsonInputDefinition,
    PageRange,
    Template,
)


def asset(file: str) -> bytes:
    """Load asset file."""
    path = Path(__file__).parent.parent.parent.parent.parent / "assets" / file
    return path.read_bytes()


def template_file() -> bytes:
    """Load E2E test template."""
    path = (
        Path(__file__).parent.parent.parent.parent.parent
        / "e2e-tests"
        / "template"
        / "oicana-e2e-test-x.y.z.zip"
    )
    return path.read_bytes()


def test_development() -> None:
    """Test compilation in development mode with no inputs."""
    template_bytes = template_file()
    template = Template(template_bytes)

    try:
        image = template.export(
            export={"format": "png", "pixelsPerPt": 1.0},
            mode=CompilationMode.DEVELOPMENT,
        )

        output_dir = Path(__file__).parent / "testOutput"
        output_dir.mkdir(exist_ok=True)
        (output_dir / "development.png").write_bytes(image)
    finally:
        template.cleanup()


def test_production() -> None:
    """Test compilation in production mode with required inputs."""
    template_bytes = template_file()
    template = Template(template_bytes)

    try:
        blob = asset("inputs/input.txt")
        json_data = asset("inputs/input.json")

        blob_inputs = {
            "development-blob": BlobInput(
                data=blob,
                metadata={"image_format": "jpeg", "foo": 43, "bar": ["input", "two"]},
            )
        }
        json_inputs = {"development-json": json_data.decode()}

        image = template.export(
            json_inputs=json_inputs,
            blob_inputs=blob_inputs,
            export={"format": "png", "pixelsPerPt": 1.0},
        )

        output_dir = Path(__file__).parent / "testOutput"
        output_dir.mkdir(exist_ok=True)
        (output_dir / "production.png").write_bytes(image)
    finally:
        template.cleanup()


def test_all_inputs() -> None:
    """Test compilation with all possible inputs."""
    template_bytes = template_file()
    template = Template(template_bytes)

    try:
        blob = asset("inputs/input.txt")
        json_data = asset("inputs/input.json")

        blob_inputs = {
            "default-blob": BlobInput(
                data=blob,
                metadata={"image_format": "jpeg", "foo": 42, "bar": ["input", "two"]},
            ),
            "development-blob": BlobInput(
                data=blob,
                metadata={"image_format": "jpeg", "foo": 43, "bar": ["input", "two"]},
            ),
            "both-blob": BlobInput(
                data=blob,
                metadata={"image_format": "jpeg", "foo": 44, "bar": ["input", "two"]},
            ),
        }
        json_inputs = {
            "default-json": json_data.decode(),
            "development-json": json_data.decode(),
            "both-json": json_data.decode(),
        }

        image = template.export(
            json_inputs=json_inputs,
            blob_inputs=blob_inputs,
            export={"format": "png", "pixelsPerPt": 1.0},
        )

        output_dir = Path(__file__).parent / "testOutput"
        output_dir.mkdir(exist_ok=True)
        (output_dir / "all-inputs.png").write_bytes(image)
    finally:
        template.cleanup()


def test_manifest() -> None:
    """The typed manifest carries the package section and the Oicana config."""
    template = Template(template_file())

    try:
        manifest = template.manifest()

        assert manifest.package.name == "oicana-e2e-test"
        assert manifest.package.version == "0.1.0"
        assert manifest.oicana.manifest_version == 1
        assert manifest.oicana.validate_json_inputs_by_default
        assert manifest.oicana.export.pdf.standards == ["a-3b"]
        assert manifest.oicana.fonts.require == []

        json_input = next(
            input for input in manifest.oicana.inputs if input.key == "development-json"
        )
        assert isinstance(json_input, JsonInputDefinition)
        assert json_input.schema == "input.schema.json"
        assert json_input.development == "development.json"
        assert json_input.default is None
        assert json_input.validate

        blob_input = next(input for input in manifest.oicana.inputs if input.key == "default-blob")
        assert isinstance(blob_input, BlobInputDefinition)
        assert blob_input.default is not None
        assert blob_input.default.file == "default.txt"
        assert blob_input.default.meta is not None
        assert blob_input.default.meta["image_format"] == "png"
        assert blob_input.development is None
    finally:
        template.cleanup()


def test_explicit_development_mode_allows_compile_with_empty_inputs() -> None:
    """Test that development mode allows compilation with empty inputs."""
    template_bytes = template_file()
    template = Template(template_bytes)

    try:
        template.export(
            export={"format": "png", "pixelsPerPt": 1.0},
            mode=CompilationMode.DEVELOPMENT,
        )
    finally:
        template.cleanup()


def test_compile_defaults_to_production_mode() -> None:
    """Test that compile defaults to production mode and fails without inputs."""
    template_bytes = template_file()
    template = Template(template_bytes)

    try:
        with pytest.raises(Exception, match="No value for the required input"):
            template.export(export={"format": "png", "pixelsPerPt": 1.0})
    finally:
        template.cleanup()


def test_can_control_compilation_mode_when_registering() -> None:
    """Test that compilation mode can be set during template registration."""
    template_bytes = template_file()

    with pytest.raises(Exception, match="No value for the required input"):
        Template(template_bytes, mode=CompilationMode.PRODUCTION)


def test_context_manager() -> None:
    """Test that template works as context manager."""
    template_bytes = template_file()

    with Template(template_bytes) as template:
        image = template.export(
            export={"format": "png", "pixelsPerPt": 1.0},
            mode=CompilationMode.DEVELOPMENT,
        )
        assert len(image) > 0


def test_compiled_document_handle_survives_template_cleanup() -> None:
    """A compiled document handle stays usable for every format and page range
    after its originating template has been cleaned up."""
    template_bytes = template_file()
    template = Template(template_bytes)

    document = template.compile(mode=CompilationMode.DEVELOPMENT)

    template.cleanup()

    assert len(document.pages) > 0
    first_page = PageRange.single(0)

    pdf = document.export_pdf(pages=first_page)
    assert pdf[:4] == b"%PDF"

    png = document.export({"format": "png", "pixelsPerPt": 1.0}, pages=first_page)
    assert png[:4] == b"\x89PNG"

    svg = document.export_svg(pages=first_page)
    assert b"<svg" in svg

    first_page_png = document.export_png(1.0, pages=PageRange.single(0))
    assert first_page_png[:4] == b"\x89PNG"

    document.close()


def test_native_calls_release_the_gil() -> None:
    template_bytes = template_file()

    with Template(template_bytes) as template:
        document = template.compile(mode=CompilationMode.DEVELOPMENT)
        window = {}

        def export() -> None:
            window["start"] = time.monotonic()
            # A single native call, made slow via a high raster resolution.
            _ = document.export_png(pixels_per_pt=8.0)
            window["end"] = time.monotonic()

        thread = threading.Thread(target=export)
        thread.start()
        ticks = []
        while thread.is_alive():
            ticks.append(time.monotonic())
            time.sleep(0.001)
        thread.join()
        document.close()

        # Ignore ticks near the edges: before the fix the background thread
        # could still be preempted between recording the timestamps and
        # entering/leaving the native call.
        duration = window["end"] - window["start"]
        margin = duration / 4
        mid_ticks = [t for t in ticks if window["start"] + margin < t < window["end"] - margin]
        assert mid_ticks, (
            "main thread never ran while export_document was in flight;"
            "the native call does not release the GIL"
        )
