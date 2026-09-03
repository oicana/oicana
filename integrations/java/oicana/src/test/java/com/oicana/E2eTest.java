package com.oicana;

import com.oicana.manifest.BlobInputDefinition;
import com.oicana.manifest.InputDefinition;
import com.oicana.manifest.JsonInputDefinition;
import com.oicana.manifest.TemplateManifest;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.*;

/**
 * E2E tests for the Oicana Java integration.
 *
 * These tests compile the e2e test template with specific inputs and write
 * PNG outputs to testOutput/. The CI pipeline then uses test_compare to
 * verify these outputs match the reference images pixel-by-pixel.
 */
class E2eTest {

    private static final Path PROJECT_ROOT = Path.of("").toAbsolutePath()
            .getParent().getParent().getParent();

    private static byte[] asset(String file) throws IOException {
        return Files.readAllBytes(PROJECT_ROOT.resolve("assets").resolve(file));
    }

    private static byte[] templateFile() throws IOException {
        return Files.readAllBytes(
                PROJECT_ROOT.resolve("e2e-tests/template/oicana-e2e-test-x.y.z.zip"));
    }

    private static void writeOutput(String filename, byte[] data) throws IOException {
        Path outputDir = Path.of("testOutput");
        Files.createDirectories(outputDir);
        Files.write(outputDir.resolve(filename), data);
    }

    @Test
    void manifestExposesThePackageSectionAndTheOicanaConfig() throws IOException {
        try (Template template = new Template(templateFile())) {
            TemplateManifest manifest = template.manifest();

            assertEquals("oicana-e2e-test", manifest.packageInfo().name());
            assertEquals("0.1.0", manifest.packageInfo().version());
            assertEquals(1, manifest.oicana().manifestVersion());
            assertTrue(manifest.oicana().validateJsonInputsByDefault());
            assertEquals(java.util.List.of("a-3b"), manifest.oicana().export().pdf().standards());
            assertEquals(java.util.List.of(), manifest.oicana().fonts().require());

            InputDefinition json = manifest.oicana().inputs().stream()
                    .filter(input -> input.key().equals("development-json"))
                    .findFirst()
                    .orElseThrow();
            assertInstanceOf(JsonInputDefinition.class, json);
            JsonInputDefinition jsonInput = (JsonInputDefinition) json;
            assertEquals("input.schema.json", jsonInput.schema());
            assertEquals("development.json", jsonInput.development());
            assertNull(jsonInput.defaultValue());
            assertTrue(jsonInput.validate());

            InputDefinition blob = manifest.oicana().inputs().stream()
                    .filter(input -> input.key().equals("default-blob"))
                    .findFirst()
                    .orElseThrow();
            assertInstanceOf(BlobInputDefinition.class, blob);
            BlobInputDefinition blobInput = (BlobInputDefinition) blob;
            assertEquals("default.txt", blobInput.defaultValue().file());
            assertEquals("png", blobInput.defaultValue().meta().get("image_format"));
            assertEquals(5L, blobInput.defaultValue().meta().get("foo"));
            assertNull(blobInput.development());
        }
    }

    @Test
    void development() throws IOException {
        try (var template = new Template(templateFile())) {
            byte[] image = template.export(
                    Map.of(),
                    Map.of(),
                    ExportFormat.png(1.0f),
                    CompilationMode.DEVELOPMENT
            );
            assertNotNull(image);
            assertTrue(image.length > 0);
            writeOutput("development.png", image);
        }
    }

    @Test
    void production() throws IOException {
        byte[] blob = asset("inputs/input.txt");
        String json = new String(asset("inputs/input.json"));

        try (var template = new Template(templateFile())) {
            byte[] image = template.export(
                    Map.of("development-json", json),
                    Map.of("development-blob", new BlobInput(
                            blob,
                            Map.of("image_format", "jpeg", "foo", 43, "bar", new Object[]{"input", "two"})
                    )),
                    ExportFormat.png(1.0f),
                    CompilationMode.PRODUCTION
            );
            assertNotNull(image);
            assertTrue(image.length > 0);
            writeOutput("production.png", image);
        }
    }

    @Test
    void allInputs() throws IOException {
        byte[] blob = asset("inputs/input.txt");
        String json = new String(asset("inputs/input.json"));

        try (var template = new Template(templateFile())) {
            byte[] image = template.export(
                    Map.of(
                            "default-json", json,
                            "development-json", json,
                            "both-json", json
                    ),
                    Map.of(
                            "default-blob", new BlobInput(
                                    blob,
                                    Map.of("image_format", "jpeg", "foo", 42, "bar", new Object[]{"input", "two"})
                            ),
                            "development-blob", new BlobInput(
                                    blob,
                                    Map.of("image_format", "jpeg", "foo", 43, "bar", new Object[]{"input", "two"})
                            ),
                            "both-blob", new BlobInput(
                                    blob,
                                    Map.of("image_format", "jpeg", "foo", 44, "bar", new Object[]{"input", "two"})
                            )
                    ),
                    ExportFormat.png(1.0f),
                    CompilationMode.PRODUCTION
            );
            assertNotNull(image);
            assertTrue(image.length > 0);
            writeOutput("all-inputs.png", image);
        }
    }

    @Test
    void explicitDevelopmentModeAllowsCompileWithEmptyInputs() throws IOException {
        try (var template = new Template(templateFile())) {
            byte[] image = template.export(
                    Map.of(),
                    Map.of(),
                    ExportFormat.png(1.0f),
                    CompilationMode.DEVELOPMENT
            );
            assertNotNull(image);
            assertTrue(image.length > 0);
        }
    }

    @Test
    void compileDefaultsToProductionMode() throws IOException {
        try (var template = new Template(templateFile())) {
            assertThrows(OicanaException.class, () -> template.export());
        }
    }

    @Test
    void canControlCompilationModeWhenRegistering() throws IOException {
        byte[] bytes = templateFile();
        assertThrows(OicanaException.class, () ->
                new Template(bytes, Map.of(), Map.of(), CompilationMode.PRODUCTION));
    }

    @Test
    void compiledDocumentHandleSurvivesTemplateClose() throws IOException {
        Template template = new Template(templateFile());

        CompiledDocument document =
                template.compile(Map.of(), Map.of(), CompilationMode.DEVELOPMENT);

        template.close();

        assertTrue(document.pageCount() > 0);
        PageRange firstPage = PageRange.single(0);

        byte[] pdf = document.exportPdf(firstPage);
        assertEquals("%PDF", new String(pdf, 0, 4, StandardCharsets.US_ASCII));

        byte[] png = document.export(ExportFormat.png(1.0f), firstPage);
        assertEquals((byte) 0x89, png[0]);
        assertEquals((byte) 'P', png[1]);
        assertEquals((byte) 'N', png[2]);
        assertEquals((byte) 'G', png[3]);

        byte[] svg = document.exportSvg(firstPage);
        assertTrue(new String(svg, StandardCharsets.UTF_8).contains("<svg"));

        byte[] firstPagePng = document.exportPng(1.0f, PageRange.single(0));
        assertEquals((byte) 0x89, firstPagePng[0]);

        document.close();
    }

    @Test
    void refusesTemplatePackedByNewerOicana() throws IOException {
        byte[] templateFile = asset("templates/future-manifest-0.1.0.zip");

        Exception error = assertThrows(Exception.class, () -> new Template(templateFile));

        assertTrue(error.getMessage().contains("manifest_version 99"),
                "unexpected message: " + error.getMessage());
    }
}
