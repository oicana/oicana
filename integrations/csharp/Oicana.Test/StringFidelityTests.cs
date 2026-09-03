using System.IO.Compression;
using System.Text;
using System.Text.Json.Nodes;
using AwesomeAssertions;
using Oicana.Inputs;
using Oicana.Interop;
using CompilationMode = Oicana.Config.CompilationMode;

namespace Oicana.Test;

public class StringFidelityTests
{
    private const string MinimalManifest = """
        [package]
        name = "fidelity-test"
        version = "0.1.0"
        entrypoint = "main.typ"

        [tool.oicana]
        manifest_version = 1
        """;

    private static byte[] PackTemplate(string manifest, string mainTypst)
    {
        using var stream = new MemoryStream();
        using (var zip = new ZipArchive(stream, ZipArchiveMode.Create, leaveOpen: true))
        {
            foreach (var (name, content) in new[] { ("typst.toml", manifest), ("main.typ", mainTypst) })
            {
                var entry = zip.CreateEntry(name, CompressionLevel.NoCompression);
                using var entryStream = entry.Open();
                var bytes = Encoding.UTF8.GetBytes(content);
                entryStream.Write(bytes, 0, bytes.Length);
            }
        }

        return stream.ToArray();
    }

    [Fact]
    public void CompileErrorContainsBackslashPathVerbatim()
    {
        var template = PackTemplate(
            MinimalManifest,
            """#assert(false, message: "see C:\\path\\file.typ")""");

        var act = () => new Template(template);

        var message = act.Should().Throw<OicanaException>().Which.Message;
        message.Should().Contain(@"see C:\path\file.typ",
            "the evaluated assert message contains single backslashes");
        message.Should().Contain(@"""see C:\\path\\file.typ""",
            "the source snippet shows the raw literal with double backslashes");
        message.Should().NotContain("Failed to read error message",
            "the diagnostic must not be replaced by the fallback message");
        message.Should().StartWith("Compilation failed:");
    }

    [Fact]
    public void CompileErrorKeepsRealNewlinesUnescaped()
    {
        var template = PackTemplate(
            MinimalManifest,
            """#assert(false, message: "boom")""");

        var act = () => new Template(template);

        var message = act.Should().Throw<OicanaException>().Which.Message;
        message.Should().Contain("boom");
        message.Should().Contain("\n", "codespan diagnostics are multi-line");
        message.Should().NotContain("\\n", "newlines must not arrive as escape sequences");
    }

    [Fact]
    public void SourceRoundTripsBackslashSequencesVerbatim()
    {
        var mainTypst = "// literal escapes: \\n \\t \\\\ end\nHello";
        using var template = new Template(PackTemplate(MinimalManifest, mainTypst));

        var source = template.Source("main.typ");

        source.Should().Be(mainTypst, "template sources must round-trip byte-for-byte");
    }

    [Fact]
    public void ManifestJsonRemainsParseable()
    {
        var manifest = MinimalManifest + "\n" + """
            [[tool.oicana.inputs]]
            type = "json"
            key = 'quo"te'
            required = false
            """;
        using var template = new Template(PackTemplate(manifest, "Hello"));

        var parsed = template.Manifest();

        parsed.Oicana.Inputs[0].Key.Should().Be("quo\"te");
    }

    [Fact]
    public void WarningsArriveVerbatim()
    {
        var mainTypst = "#set text(font: \"NonexistentFontFidelity\")\nContent";
        var template = PackTemplate(MinimalManifest, mainTypst);

        using var registered = new Template(template);
        using var document = registered.Compile(
            new Dictionary<string, JsonNode>(),
            new Dictionary<string, BlobInput>(),
            new Oicana.Config.CompilationOptions(CompilationMode.Development));

        document.Warnings.Should().NotBeNull();
        document.Warnings.Should().Contain("NonexistentFontFidelity");
        document.Warnings.Should().NotContain("\\n", "newlines must not arrive as escape sequences");
    }
}
