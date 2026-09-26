package com.oicana;

import org.junit.jupiter.api.Test;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Map;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Tests for the one-shot {@link Template#exportOnce} API.
 */
class ExportOnceTest {

    private static final Path PROJECT_ROOT = Path.of("").toAbsolutePath()
            .getParent().getParent().getParent();

    private static final String MINIMAL_MANIFEST = """
            [package]
            name = "export-once-test"
            version = "0.1.0"
            entrypoint = "main.typ"

            [tool.oicana]
            manifest_version = 1
            """;

    private static byte[] templateFile() throws IOException {
        return Files.readAllBytes(
                PROJECT_ROOT.resolve("e2e-tests/template/oicana-e2e-test-x.y.z.zip"));
    }

    private static byte[] packTemplate(String manifest, String mainTypst) throws IOException {
        var stream = new ByteArrayOutputStream();
        try (var zip = new ZipOutputStream(stream)) {
            zip.setMethod(ZipOutputStream.STORED);
            for (var entry : Map.of("typst.toml", manifest, "main.typ", mainTypst).entrySet()) {
                byte[] content = entry.getValue().getBytes(StandardCharsets.UTF_8);
                var zipEntry = new ZipEntry(entry.getKey());
                zipEntry.setSize(content.length);
                var crc = new java.util.zip.CRC32();
                crc.update(content);
                zipEntry.setCrc(crc.getValue());
                zip.putNextEntry(zipEntry);
                zip.write(content);
                zip.closeEntry();
            }
        }
        return stream.toByteArray();
    }

    @Test
    void exportsWithoutWarnings() throws IOException {
        ExportOnceResult result = Template.exportOnce(
                templateFile(),
                Map.of(),
                Map.of(),
                ExportFormat.pdf(),
                CompilationMode.DEVELOPMENT
        );

        assertEquals("%PDF", new String(result.document(), 0, 4, StandardCharsets.US_ASCII));
        assertTrue(result.warnings().isEmpty());
    }

    @Test
    void surfacesWarnings() throws IOException {
        byte[] template = packTemplate(
                MINIMAL_MANIFEST,
                "#set text(font: \"NonexistentFontExportOnce\")\nContent");

        ExportOnceResult result = Template.exportOnce(
                template,
                Map.of(),
                Map.of(),
                ExportFormat.svg(),
                CompilationMode.DEVELOPMENT
        );

        assertTrue(new String(result.document(), StandardCharsets.UTF_8).contains("<svg"));
        assertTrue(result.warnings().isPresent());
        assertTrue(result.warnings().get().contains("NonexistentFontExportOnce"));
    }

    @Test
    void enforcesZipLimits() throws IOException {
        byte[] bytes = templateFile();
        var exception = assertThrows(OicanaException.class, () -> Template.exportOnce(
                bytes,
                Map.of(),
                Map.of(),
                ExportFormat.pdf(),
                CompilationMode.DEVELOPMENT,
                null,
                new ZipLimits(1L, null)
        ));
        assertTrue(exception.getMessage().contains("entries"));
    }

    @Test
    void registrationEnforcesZipLimits() throws IOException {
        byte[] bytes = templateFile();
        var exception = assertThrows(OicanaException.class, () ->
                new Template(bytes, Map.of(), Map.of(), CompilationMode.DEVELOPMENT,
                        new ZipLimits(1L, null)));
        assertTrue(exception.getMessage().contains("entries"));
    }

    @Test
    void rejectsNegativeZipLimits() {
        var entries = assertThrows(IllegalArgumentException.class,
                () -> new ZipLimits(-1L, null));
        assertTrue(entries.getMessage().contains("maxEntries"));

        var bytes = assertThrows(IllegalArgumentException.class,
                () -> new ZipLimits(null, -8L));
        assertTrue(bytes.getMessage().contains("maxTotalDecompressedBytes"));
    }

    @Test
    void acceptsZeroAndNullZipLimits() {
        assertEquals(0L, new ZipLimits(0L, null).maxEntries());
        assertNull(new ZipLimits(null, null).maxEntries());
    }

    @Test
    void diagnosticColorConfigurationSucceeds() {
        Configuration.configureDiagnosticColor(DiagnosticColor.ANSI);
        Configuration.configureDiagnosticColor(DiagnosticColor.NONE);
    }
}
