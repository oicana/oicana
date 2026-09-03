"""Oicana Python integration."""

from __future__ import annotations

import json
import math
import uuid
from typing import TYPE_CHECKING

from oicana_native import (
    BlobWithMetadata,
    compile_template,
    document_pages,
    export_document,
    get_file,
    get_source,
    get_warnings,
    manifest,
    register_template,
    remove_document,
    remove_world,
)
from oicana_native import (
    CompilationMode as NativeCompilationMode,
)
from oicana_native import (
    clear_fonts as _clear_fonts,
)
from oicana_native import (
    configure_automatic_cache_eviction as _configure_automatic_cache_eviction,
)
from oicana_native import (
    configure_diagnostic_color as _configure_diagnostic_color,
)
from oicana_native import (
    evict_cache as _evict_cache,
)
from oicana_native import (
    export_once as _export_once,
)
from oicana_native import (
    register_font_paths as _register_font_paths,
)
from oicana_native import (
    register_fonts as _register_fonts,
)
from oicana_native import (
    registered_fonts as _registered_fonts,
)
from oicana_native import (
    set_validate_inputs as _set_validate_inputs,
)

from .types import (
    BlobInput,
    CompilationMode,
    DiagnosticColor,
    ExportFormat,
    ExportOnceResult,
    PageRange,
    PageSize,
    RegisteredFont,
    TemplateManifest,
    ZipLimits,
)

if TYPE_CHECKING:
    import os
    from collections.abc import Iterable


def _serialize_export_format(export: ExportFormat) -> str:
    """Serialize an export format for the native export calls.

    Raises:
        ValueError: If ``pixelsPerPt`` is not a positive, finite number. ``json.dumps``
            would otherwise emit ``NaN``/``Infinity``, which is not valid JSON.
    """
    if export["format"] == "png":
        pixels_per_pt = export["pixelsPerPt"]
        if not math.isfinite(pixels_per_pt) or pixels_per_pt <= 0:
            raise ValueError(f"pixelsPerPt must be a positive, finite number, got {pixels_per_pt}")
    return json.dumps(export)


def _serialize_page_range(pages: PageRange | None) -> str | None:
    """Serialize a page range for the native ``export_document`` call.

    ``None`` means "the whole document".
    """
    if pages is None:
        return None
    payload: dict[str, int] = {}
    if pages.start is not None:
        payload["start"] = pages.start
    if pages.end is not None:
        payload["end"] = pages.end
    return json.dumps(payload)


class Template:
    """Oicana template for PDF generation.

    Example:
        >>> with Template(template_bytes) as template:
        ...     pdf = template.export(
        ...         json_inputs={"name": '{"value": "Alice"}'},
        ...         export={"format": "pdf"}
        ...     )
    """

    def __init__(
        self,
        template: bytes,
        *,
        json_inputs: dict[str, str] | None = None,
        blob_inputs: dict[str, BlobInput] | None = None,
        mode: CompilationMode = CompilationMode.DEVELOPMENT,
        limits: ZipLimits | None = None,
    ) -> None:
        """Initialize template.

        Args:
            template: Template zip file bytes
            json_inputs: Initial JSON inputs (key -> JSON string)
            blob_inputs: Initial blob inputs
            mode: Compilation mode (development/production)
            limits: Limits for reading the template zip (defaults apply when None)
        """
        self._template_id = str(uuid.uuid4())

        native_mode = (
            NativeCompilationMode.Production
            if mode == CompilationMode.PRODUCTION
            else NativeCompilationMode.Development
        )

        native_json = json_inputs if json_inputs is not None else {}

        native_blobs = {}
        if blob_inputs:
            for key, blob in blob_inputs.items():
                meta_str = json.dumps(blob.metadata) if blob.metadata else "{}"
                native_blobs[key] = BlobWithMetadata(blob.data, meta_str)

        doc_id = register_template(
            self._template_id,
            template,
            native_json,
            native_blobs,
            native_mode,
            limits.max_entries if limits else None,
            limits.max_total_decompressed_bytes if limits else None,
        )
        self._last_warnings: str | None = get_warnings(doc_id)
        remove_document(doc_id)

    @property
    def warnings(self) -> str | None:
        """Warnings from the most recent compilation, or ``None`` if there were none."""
        return self._last_warnings

    def export(
        self,
        *,
        json_inputs: dict[str, str] | None = None,
        blob_inputs: dict[str, BlobInput] | None = None,
        export: ExportFormat = {"format": "pdf"},  # type: ignore[typeddict-item]
        mode: CompilationMode = CompilationMode.PRODUCTION,
        pages: PageRange | None = None,
    ) -> bytes:
        """Compile template and export to the given format.

        Args:
            json_inputs: JSON inputs
            blob_inputs: Blob inputs
            export: Export format and configuration (pdf/png/svg)
            mode: Compilation mode
            pages: 0-based, inclusive page range (defaults to the whole document)

        Returns:
            Compiled document bytes
        """
        doc_id = self._compile_to_document_id(json_inputs, blob_inputs, mode)
        try:
            result = export_document(
                doc_id, _serialize_export_format(export), _serialize_page_range(pages)
            )
        finally:
            remove_document(doc_id)

        return bytes(result)

    def export_pdf(
        self,
        *,
        json_inputs: dict[str, str] | None = None,
        blob_inputs: dict[str, BlobInput] | None = None,
        mode: CompilationMode = CompilationMode.PRODUCTION,
        pages: PageRange | None = None,
    ) -> bytes:
        """Compile the template and export it to PDF in a single call.

        Tagging will be automatically turned off when exporting a subset of pages.

        Args:
            json_inputs: JSON inputs
            blob_inputs: Blob inputs
            mode: Compilation mode
            pages: 0-based, inclusive page range (defaults to the whole document)
        """
        return self.export(
            json_inputs=json_inputs,
            blob_inputs=blob_inputs,
            export={"format": "pdf"},
            mode=mode,
            pages=pages,
        )

    def export_png(
        self,
        *,
        json_inputs: dict[str, str] | None = None,
        blob_inputs: dict[str, BlobInput] | None = None,
        mode: CompilationMode = CompilationMode.PRODUCTION,
        pixels_per_pt: float = 1.0,
        pages: PageRange | None = None,
    ) -> bytes:
        """Compile the template and export it to PNG in a single call.

        Multiple pages are merged into a single, vertically stacked image.

        Args:
            json_inputs: JSON inputs
            blob_inputs: Blob inputs
            mode: Compilation mode
            pixels_per_pt: Resolution in pixels per point (defaults to 1.0)
            pages: 0-based, inclusive page range (defaults to the whole document)
        """
        return self.export(
            json_inputs=json_inputs,
            blob_inputs=blob_inputs,
            export={"format": "png", "pixelsPerPt": pixels_per_pt},
            mode=mode,
            pages=pages,
        )

    def export_svg(
        self,
        *,
        json_inputs: dict[str, str] | None = None,
        blob_inputs: dict[str, BlobInput] | None = None,
        mode: CompilationMode = CompilationMode.PRODUCTION,
        pages: PageRange | None = None,
    ) -> bytes:
        """Compile the template and export it to SVG in a single call.

        Args:
            json_inputs: JSON inputs
            blob_inputs: Blob inputs
            mode: Compilation mode
            pages: 0-based, inclusive page range (defaults to the whole document)
        """
        return self.export(
            json_inputs=json_inputs,
            blob_inputs=blob_inputs,
            export={"format": "svg"},
            mode=mode,
            pages=pages,
        )

    @staticmethod
    def export_once(
        template: bytes,
        *,
        json_inputs: dict[str, str] | None = None,
        blob_inputs: dict[str, BlobInput] | None = None,
        export: ExportFormat = {"format": "pdf"},  # type: ignore[typeddict-item]
        mode: CompilationMode = CompilationMode.PRODUCTION,
        pages: PageRange | None = None,
        limits: ZipLimits | None = None,
    ) -> ExportOnceResult:
        """Compile and export a template in a single native call, without caching.

        Nothing is registered and no warm-up compilation runs, so this is the
        fastest way to render a template exactly once. For repeated exports of
        the same template, create a :class:`Template` instance instead.

        Args:
            template: Template zip file bytes
            json_inputs: JSON inputs
            blob_inputs: Blob inputs
            export: Export format and configuration (pdf/png/svg)
            mode: Compilation mode
            pages: 0-based, inclusive page range (defaults to the whole document)
            limits: Limits for reading the template zip (defaults apply when None)

        Returns:
            The exported document and any compilation warnings.
        """
        native_mode = (
            NativeCompilationMode.Production
            if mode == CompilationMode.PRODUCTION
            else NativeCompilationMode.Development
        )

        native_json = json_inputs if json_inputs is not None else {}

        native_blobs = {}
        if blob_inputs:
            for key, blob in blob_inputs.items():
                meta_str = json.dumps(blob.metadata) if blob.metadata else "{}"
                native_blobs[key] = BlobWithMetadata(blob.data, meta_str)

        document, warnings = _export_once(
            template,
            native_json,
            native_blobs,
            native_mode,
            _serialize_export_format(export),
            _serialize_page_range(pages),
            limits.max_entries if limits else None,
            limits.max_total_decompressed_bytes if limits else None,
        )
        return ExportOnceResult(document=bytes(document), warnings=warnings)

    def compile(
        self,
        *,
        json_inputs: dict[str, str] | None = None,
        blob_inputs: dict[str, BlobInput] | None = None,
        mode: CompilationMode = CompilationMode.PRODUCTION,
    ) -> CompiledDocument:
        """Compile the template and return a handle to the compiled document.

        Unlike :meth:`export`, the document is kept in memory so it can be
        exported one or more times without re-compiling. Use the result as
        a context manager or call ``close()`` to free it.

        Args:
            json_inputs: JSON inputs
            blob_inputs: Blob inputs
            mode: Compilation mode

        Returns:
            A :class:`CompiledDocument` handle.
        """
        doc_id = self._compile_to_document_id(json_inputs, blob_inputs, mode)
        return CompiledDocument(doc_id)

    def _compile_to_document_id(
        self,
        json_inputs: dict[str, str] | None,
        blob_inputs: dict[str, BlobInput] | None,
        mode: CompilationMode,
    ) -> str:
        native_mode = (
            NativeCompilationMode.Production
            if mode == CompilationMode.PRODUCTION
            else NativeCompilationMode.Development
        )

        native_json = json_inputs if json_inputs is not None else {}

        native_blobs = {}
        if blob_inputs:
            for key, blob in blob_inputs.items():
                meta_str = json.dumps(blob.metadata) if blob.metadata else "{}"
                native_blobs[key] = BlobWithMetadata(blob.data, meta_str)

        doc_id: str = compile_template(
            self._template_id,
            native_json,
            native_blobs,
            native_mode,
        )
        self._last_warnings = get_warnings(doc_id)
        return doc_id

    def manifest(self) -> TemplateManifest:
        """Get the template's manifest.

        Returns:
            The Typst package section and the Oicana configuration of the
            template, including its input definitions
        """
        return TemplateManifest.from_json(json.loads(manifest(self._template_id)))

    def source(self, path: str) -> str:
        """Get source file content.

        Args:
            path: File path in template

        Returns:
            File content as string
        """
        return get_source(self._template_id, path)  # type: ignore[no-any-return]

    def file(self, path: str) -> bytes:
        """Get binary file content.

        Args:
            path: File path in template

        Returns:
            File content as bytes
        """
        return bytes(get_file(self._template_id, path))

    def set_validate_inputs(self, validate: bool) -> None:
        """Enable or disable JSON schema validation for this template.

        When enabled (the default), JSON inputs are validated against their schemas
        before compilation.

        Args:
            validate: Whether to validate inputs against their JSON schemas.
        """
        _set_validate_inputs(self._template_id, validate)

    def cleanup(self) -> None:
        """Clean up cached resources."""
        remove_world(self._template_id)

    def __enter__(self) -> Template:
        """Context manager entry."""
        return self

    def __exit__(self, *args: object) -> None:
        """Context manager exit with cleanup."""
        self.cleanup()

    def __del__(self) -> None:
        """Destructor cleanup."""
        try:
            self.cleanup()
        except Exception:
            pass  # Best effort cleanup


class CompiledDocument:
    """A compiled document kept in memory so its pages can be exported on demand.

    Obtain one via :meth:`Template.compile`. Use it as a context manager
    (``with template.compile() as document: ...``) or call
    :meth:`close` to release the underlying document.

    Example:
        >>> with template.compile(json_inputs={...}) as document:
        ...     for index, _page in enumerate(document.pages):
        ...         png = document.export_png(2.0, pages=PageRange.single(index))
    """

    def __init__(self, document_id: str) -> None:
        """Wrap an already-compiled document. Use Template.compile()."""
        self._document_id: str | None = document_id
        self.pages: list[PageSize] = [
            PageSize(width=page["width"], height=page["height"])
            for page in json.loads(document_pages(document_id))
        ]
        #: Warnings produced by the compilation of this document, or ``None``.
        self.warnings: str | None = get_warnings(document_id)

    def export(
        self,
        export: ExportFormat = {"format": "pdf"},  # type: ignore[typeddict-item]
        pages: PageRange | None = None,
    ) -> bytes:
        """Export the document in the given format (defaults to PDF).

        Args:
            export: Export format and configuration (pdf/png/svg)
            pages: 0-based, inclusive page range (defaults to the whole document)
        """
        if self._document_id is None:
            raise RuntimeError("CompiledDocument has already been closed")
        return bytes(
            export_document(
                self._document_id, _serialize_export_format(export), _serialize_page_range(pages)
            )
        )

    def export_pdf(self, pages: PageRange | None = None) -> bytes:
        """Export the document to PDF, optionally restricted to a range.

        Tagging will be automatically turned off when exporting a subset of pages.

        Args:
            pages: 0-based, inclusive page range (defaults to the whole document)
        """
        return self.export({"format": "pdf"}, pages=pages)

    def export_png(self, pixels_per_pt: float = 1.0, pages: PageRange | None = None) -> bytes:
        """Export the document to PNG, optionally restricted to a range.

        Multiple pages are merged into a single, vertically stacked image.

        Args:
            pixels_per_pt: Resolution in pixels per point (defaults to 1.0)
            pages: 0-based, inclusive page range (defaults to the whole document)
        """
        return self.export({"format": "png", "pixelsPerPt": pixels_per_pt}, pages=pages)

    def export_svg(self, pages: PageRange | None = None) -> bytes:
        """Export the document to SVG, optionally restricted to a range.

        Args:
            pages: 0-based, inclusive page range (defaults to the whole document)
        """
        return self.export({"format": "svg"}, pages=pages)

    def close(self) -> None:
        """Release the cached document. The instance must not be used after."""
        if self._document_id is not None:
            remove_document(self._document_id)
            self._document_id = None

    def __enter__(self) -> CompiledDocument:
        """Context manager entry."""
        return self

    def __exit__(self, *args: object) -> None:
        """Context manager exit; releases the document."""
        self.close()

    def __del__(self) -> None:
        """Destructor cleanup."""
        try:
            self.close()
        except Exception:
            pass  # Best effort cleanup


def configure_automatic_cache_eviction(max_age: int | None) -> None:
    """Configure automatic cache eviction after each compilation.

    Args:
        max_age: Maximum age threshold, or None to disable:
            - None - Disables cache eviction (cache never cleared)
            - 0 - Clears all cache entries with every eviction
            - 1 - Keeps only entries used since the last eviction
            - n - Keeps entries used within the last n evictions
            Default is 10.
    """
    _configure_automatic_cache_eviction(max_age)


def evict_cache(max_age: int) -> None:
    """Manually evict the cache with the given age threshold.

    This directly calls the underlying eviction with the specified age,
    regardless of the configured default age.

    Args:
        max_age: Maximum age threshold for eviction.
            Entries with age >= this value will be removed.
    """
    _evict_cache(max_age)


def configure_diagnostic_color(color: DiagnosticColor) -> None:
    """Configure the coloring of compilation diagnostics like warnings and errors.

    Args:
        color: The color mode to use.
    """
    _configure_diagnostic_color(color == DiagnosticColor.ANSI)


def register_fonts(fonts: bytes | Iterable[bytes]) -> int:
    """Make fonts available to every template registered from now on.

    Args:
        fonts: The content of one or more font files. Data that holds no font
            is ignored.

    Returns:
        The number of font faces that were added.
    """
    if isinstance(fonts, (bytes, bytearray, memoryview)):
        fonts = [fonts]
    return _register_fonts([bytes(font) for font in fonts])  # type: ignore[no-any-return]


def register_font_paths(paths: str | os.PathLike[str] | Iterable[str | os.PathLike[str]]) -> int:
    """Make fonts on disk available to every template registered from now on.

    Args:
        paths: One or more paths to font files.

    Returns:
        The number of font faces that were added.
    """
    if isinstance(paths, str) or hasattr(paths, "__fspath__"):
        paths = [paths]  # type: ignore[list-item]
    return _register_font_paths([str(path) for path in paths])  # type: ignore[no-any-return,union-attr]


def registered_fonts() -> list[RegisteredFont]:
    """The font faces currently registered with :func:`register_fonts` or
    :func:`register_font_paths`."""
    return [RegisteredFont(family=family, path=path) for family, path in _registered_fonts()]


def clear_fonts() -> None:
    """Drop all registered fonts.

    Templates that are already registered keep the fonts they were created with.
    """
    _clear_fonts()
