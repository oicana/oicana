"""Type definitions for Oicana."""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from typing import Any, Literal, TypedDict, Union


class CompilationMode(Enum):
    """Template compilation mode."""

    PRODUCTION = "production"
    DEVELOPMENT = "development"


class DiagnosticColor(Enum):
    """Color mode for compilation diagnostics."""

    NONE = "none"
    ANSI = "ansi"


class ExportFormatPdf(TypedDict):
    """PDF export format."""

    format: Literal["pdf"]


class ExportFormatSvg(TypedDict):
    """SVG export format."""

    format: Literal["svg"]


class ExportFormatPng(TypedDict):
    """PNG export format."""

    format: Literal["png"]
    pixelsPerPt: float


ExportFormat = Union[ExportFormatPdf, ExportFormatSvg, ExportFormatPng]


@dataclass
class PageRange:
    """A contiguous, 0-based inclusive range of document pages to export.

    Both bounds are optional; leaving one as ``None`` keeps it open.
    """

    start: int | None = None
    end: int | None = None

    @classmethod
    def single(cls, page: int) -> PageRange:
        """A range selecting exactly the page at the given 0-based index."""
        return cls(start=page, end=page)

    @classmethod
    def of(cls, start: int | None = None, end: int | None = None) -> PageRange:
        """A range with the given (optional) 0-based, inclusive bounds."""
        return cls(start=start, end=end)


@dataclass
class PageSize:
    """Size of a single document page, in typographic points (pt)."""

    width: float
    height: float


@dataclass
class BlobInput:
    """Binary blob input with optional metadata."""

    data: bytes
    metadata: dict[str, Any] | None = None


@dataclass
class ZipLimits:
    """Limits applied when reading a packed template zip.

    A ``None`` bound keeps the default (10 000 entries / 512 MiB decompressed).
    """

    max_entries: int | None = None
    max_total_decompressed_bytes: int | None = None


@dataclass
class RegisteredFont:
    """A font face made available to templates by the host."""

    #: Family name, as used in Typst's ``text(font: ...)``.
    family: str
    #: File the face was read from; ``None`` for fonts registered from memory.
    path: str | None = None


@dataclass
class ExportOnceResult:
    """Result of a one-shot template export."""

    document: bytes
    warnings: str | None


@dataclass
class JsonInputDefinition:
    """An input taking a JSON value."""

    #: Key the input is supplied and used under.
    key: str
    #: Whether a value of this input is required for compilation.
    required: bool
    #: File in the template holding the value used when none is supplied.
    #: In development mode, ``development`` takes precedence.
    default: str | None
    #: File in the template holding the value used in development mode when none is supplied.
    development: str | None
    #: File in the template holding the JSON schema of this input.
    schema: str | None
    #: Whether values are validated against the schema.
    validate: bool
    #: Discriminator of the input kind.
    type: Literal["json"] = "json"

    @classmethod
    def from_json(cls, data: dict[str, Any]) -> JsonInputDefinition:
        """Build the definition from its manifest representation."""
        return cls(
            key=data["key"],
            required=data["required"],
            default=data["default"],
            development=data["development"],
            schema=data["schema"],
            validate=data["validate"],
        )


@dataclass
class BlobFallback:
    """A blob from the template, used when no value is supplied."""

    #: File in the template holding the blob.
    file: str
    #: Metadata passed to the template along with the blob.
    meta: dict[str, Any] | None

    @classmethod
    def from_json(cls, data: dict[str, Any]) -> BlobFallback:
        """Build the fallback from its manifest representation."""
        return cls(file=data["file"], meta=data["meta"])


@dataclass
class BlobInputDefinition:
    """An input taking arbitrary bytes."""

    #: Key the input is supplied and used under.
    key: str
    #: Whether a value of this input is required for compilation.
    required: bool
    #: Blob used when no value is supplied.
    #: In development mode, ``development`` takes precedence.
    default: BlobFallback | None
    #: Blob used in development mode when no value is supplied.
    development: BlobFallback | None
    #: Discriminator of the input kind.
    type: Literal["blob"] = "blob"

    @classmethod
    def from_json(cls, data: dict[str, Any]) -> BlobInputDefinition:
        """Build the definition from its manifest representation."""
        return cls(
            key=data["key"],
            required=data["required"],
            default=BlobFallback.from_json(data["default"]) if data["default"] else None,
            development=(
                BlobFallback.from_json(data["development"]) if data["development"] else None
            ),
        )


#: An input a template declares.
InputDefinition = Union[JsonInputDefinition, BlobInputDefinition]


@dataclass
class PdfExportConfig:
    """How documents are exported to PDF."""

    #: PDF standards the export conforms to, for example ``a-3b``.
    standards: list[str]
    #: Whether the PDF is tagged for accessibility.
    tagged: bool


@dataclass
class ExportConfig:
    """How compiled documents are exported."""

    pdf: PdfExportConfig

    @classmethod
    def from_json(cls, data: dict[str, Any]) -> ExportConfig:
        """Build the configuration from its manifest representation."""
        return cls(
            pdf=PdfExportConfig(standards=data["pdf"]["standards"], tagged=data["pdf"]["tagged"])
        )


@dataclass
class FontConfig:
    """Fonts a template expects from its host."""

    #: Font families the host has to register for this template.
    require: list[str]


@dataclass
class OicanaConfig:
    """The Oicana configuration of a template."""

    #: Version of the manifest format.
    manifest_version: int
    #: The inputs the template declares, in manifest order.
    inputs: list[InputDefinition]
    #: Whether JSON inputs are validated against their schemas by default.
    validate_json_inputs_by_default: bool
    export: ExportConfig
    #: Fonts the template expects from its host.
    fonts: FontConfig

    @classmethod
    def from_json(cls, data: dict[str, Any]) -> OicanaConfig:
        """Build the configuration from its manifest representation."""
        return cls(
            manifest_version=data["manifestVersion"],
            inputs=[_input_definition_from_json(input) for input in data["inputs"]],
            validate_json_inputs_by_default=data["validateJsonInputsByDefault"],
            export=ExportConfig.from_json(data["export"]),
            fonts=FontConfig(require=data["fonts"]["require"]),
        )


def _input_definition_from_json(data: dict[str, Any]) -> InputDefinition:
    if data["type"] == "json":
        return JsonInputDefinition.from_json(data)
    return BlobInputDefinition.from_json(data)


@dataclass
class PackageInfo:
    """The Typst package a template is."""

    name: str
    version: str
    #: File the compilation starts at.
    entrypoint: str
    authors: list[str]
    license: str | None
    description: str | None
    homepage: str | None
    repository: str | None

    @classmethod
    def from_json(cls, data: dict[str, Any]) -> PackageInfo:
        """Build the package information from its manifest representation."""
        return cls(
            name=data["name"],
            version=data["version"],
            entrypoint=data["entrypoint"],
            authors=data["authors"],
            license=data["license"],
            description=data["description"],
            homepage=data["homepage"],
            repository=data["repository"],
        )


@dataclass
class TemplateManifest:
    """A template's manifest."""

    #: The Typst package section of the manifest.
    package: PackageInfo
    #: The Oicana section of the manifest.
    oicana: OicanaConfig

    @classmethod
    def from_json(cls, data: dict[str, Any]) -> TemplateManifest:
        """Build the manifest from the JSON the native module returns."""
        return cls(
            package=PackageInfo.from_json(data["package"]),
            oicana=OicanaConfig.from_json(data["oicana"]),
        )
