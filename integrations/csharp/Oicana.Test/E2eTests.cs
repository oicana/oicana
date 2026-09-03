using System.Text.Json;
using System.Text.Json.Nodes;
using AwesomeAssertions;
using Oicana.Inputs;
using Oicana.Manifest;
using Oicana.Interop;
using CompilationMode = Oicana.Config.CompilationMode;
using CompilationOptions = Oicana.Config.CompilationOptions;
using ExportFormat = Oicana.Config.ExportFormat;
using PageRange = Oicana.Config.PageRange;

namespace Oicana.Test;

public class E2ETests
{
    private readonly byte[] _templateFile = File.ReadAllBytes("../../../../../../e2e-tests/template/oicana-e2e-test-x.y.z.zip");

    public E2ETests()
    {
        Directory.CreateDirectory("e2e");
    }

    [Fact]
    public void Development()
    {
        var template = new Template(_templateFile);

        var document = template.Export(new Dictionary<string, JsonNode>(), new Dictionary<string, BlobInput>(), ExportFormat.Png(1.0f), new CompilationOptions(CompilationMode.Development));
        using var fileStream = File.Create("e2e/development.png");
        document.CopyTo(fileStream);
    }

    [Fact]
    public void Production()
    {
        var template = new Template(_templateFile);

        var blobInputs = new Dictionary<string, BlobInput>
        {
            ["development-blob"] = new BlobInput("Input"u8.ToArray(), new BlobMeta()
            {
                ImageFormat = "jpeg",
                Custom = JsonNode.Parse(
                    """
                    {
                       "bar": ["input", "two"],
                       "foo": 43
                    }
                    """)!.AsObject()
            }),
        };
        var jsonInputs = new Dictionary<string, JsonNode>
        {
            ["development-json"] = JsonSerializer.Deserialize<JsonNode>("{ \"name\": \"Input\", \"foo\": [41, \"testing\"] }")!
        };
        var document = template.Export(jsonInputs, blobInputs, ExportFormat.Png(1.0f), new CompilationOptions(CompilationMode.Production));
        using var fileStream = File.Create("e2e/production.png");
        document.CopyTo(fileStream);
    }

    [Fact]
    public void AllInputs()
    {
        var template = new Template(_templateFile);

        var blobInputs = new Dictionary<string, BlobInput>
        {
            ["default-blob"] = new BlobInput("Input"u8.ToArray(), new BlobMeta()
            {
                ImageFormat = "jpeg",
                Custom = JsonNode.Parse(
                    """
                    {
                       "bar": ["input", "two"],
                       "foo": 42
                    }
                    """)!.AsObject()
            }),
            ["development-blob"] = new BlobInput("Input"u8.ToArray(), new BlobMeta()
            {
                ImageFormat = "jpeg",
                Custom = JsonNode.Parse(
                    """
                    {
                       "bar": ["input", "two"],
                       "foo": 43
                    }
                    """)!.AsObject()
            }),
            ["both-blob"] = new BlobInput("Input"u8.ToArray(), new BlobMeta()
            {
                ImageFormat = "jpeg",
                Custom = JsonNode.Parse(
                    """
                    {
                       "bar": ["input", "two"],
                       "foo": 44
                    }
                    """)!.AsObject()
            }),
        };
        var jsonData = JsonSerializer.Deserialize<JsonNode>("{ \"name\": \"Input\", \"foo\": [41, \"testing\"] }")!;
        var jsonInputs = new Dictionary<string, JsonNode>
        {
            ["default-json"] = jsonData,
            ["development-json"] = jsonData,
            ["both-json"] = jsonData
        };

        var document = template.Export(jsonInputs, blobInputs, ExportFormat.Png(1.0f), new CompilationOptions(CompilationMode.Production));
        using var fileStream = File.Create("e2e/all-inputs.png");
        document.CopyTo(fileStream);
    }

    [Fact]
    public void GetsReadableErrors()
    {
        var template = new Template(_templateFile);
        Action act = () => template.Export(new Dictionary<string, JsonNode>(), new Dictionary<string, BlobInput>(), ExportFormat.Png(1.0f), new CompilationOptions(CompilationMode.Production));

        act.Should()
            .Throw<OicanaException>()
            .WithMessage("*No value for the required input*", "compilation in production mode should fail for inputs without default values");
    }

    [Fact]
    public void GetManifest()
    {
        var template = new Template(_templateFile);

        var manifest = template.Manifest();

        manifest.Package.Name.Should().Be("oicana-e2e-test");
        manifest.Package.Version.Should().Be("0.1.0");
        manifest.Oicana.ManifestVersion.Should().Be(1);
        manifest.Oicana.ValidateJsonInputsByDefault.Should().BeTrue();
        manifest.Oicana.Export.Pdf.Standards.Should().Equal("a-3b");
        manifest.Oicana.Fonts.Require.Should().BeEmpty();

        var inputKeys = manifest.Oicana.Inputs.Select(input => input.Key).ToList();
        inputKeys.Should().HaveCount(6);
        inputKeys.Should().Contain("default-json");
        inputKeys.Should().Contain("development-json");
        inputKeys.Should().Contain("both-json");
        inputKeys.Should().Contain("default-blob");
        inputKeys.Should().Contain("development-blob");
        inputKeys.Should().Contain("both-blob");

        manifest.Oicana.Inputs.OfType<JsonInputDefinition>().Should().HaveCount(3);
        manifest.Oicana.Inputs.OfType<BlobInputDefinition>().Should().HaveCount(3);

        var json = manifest.Oicana.Inputs.OfType<JsonInputDefinition>()
            .Single(input => input.Key == "development-json");
        json.Schema.Should().Be("input.schema.json");
        json.Development.Should().Be("development.json");
        json.Default.Should().BeNull();
        json.Validate.Should().BeTrue();

        var blob = manifest.Oicana.Inputs.OfType<BlobInputDefinition>()
            .Single(input => input.Key == "default-blob");
        blob.Default!.File.Should().Be("default.txt");
        blob.Default.Meta!["image_format"]!.GetValue<string>().Should().Be("png");
        blob.Development.Should().BeNull();
    }

    [Fact]
    public void GetSource()
    {
        var template = new Template(_templateFile);

        var source = template.Source("/main.typ");

        source.Should().NotBeNullOrEmpty();
        source.Should().Contain("#import");
    }

    [Fact]
    public void GetSourceThrowsForMissingFile()
    {
        var template = new Template(_templateFile);
        Action act = () => template.Source("/nonexistent.typ");

        act.Should().Throw<OicanaException>();
    }

    [Fact]
    public void GetFile()
    {
        var template = new Template(_templateFile);

        var file = template.File("/default.txt");

        file.Should().NotBeNullOrEmpty();
        System.Text.Encoding.UTF8.GetString(file).Should().Contain("Default");
    }

    [Fact]
    public void GetFileThrowsForMissingFile()
    {
        var template = new Template(_templateFile);
        Action act = () => template.File("/nonexistent.png");

        act.Should().Throw<OicanaException>();
    }

    [Fact]
    public void CompiledDocumentHandleSurvivesTemplateDispose()
    {
        var template = new Template(_templateFile);

        var document = template.Compile(
            new Dictionary<string, JsonNode>(),
            new Dictionary<string, BlobInput>(),
            new CompilationOptions(CompilationMode.Development));

        template.Dispose();

        document.PageCount.Should().BeGreaterThan(0);
        var firstPage = PageRange.Single(0);

        var pdf = ReadBytes(document.ExportPdf(firstPage));
        System.Text.Encoding.ASCII.GetString(pdf, 0, 4).Should().Be("%PDF");

        var png = ReadBytes(document.Export(ExportFormat.Png(1.0f), firstPage));
        png.Should().HaveCountGreaterThan(4);
        png[0].Should().Be(0x89);
        png[1].Should().Be(0x50);

        var svg = ReadBytes(document.ExportSvg(firstPage));
        System.Text.Encoding.UTF8.GetString(svg).Should().Contain("<svg");

        var firstPagePng = ReadBytes(document.ExportPng(1.0f, PageRange.Single(0)));
        firstPagePng[0].Should().Be(0x89);

        document.Dispose();
    }

    private static byte[] ReadBytes(Stream stream)
    {
        using var memory = new MemoryStream();
        stream.CopyTo(memory);
        return memory.ToArray();
    }
}
