using System.Diagnostics;
using System.Text.Json.Nodes;
using AwesomeAssertions;
using Oicana.Inputs;
using Oicana.Interop;
using CompilationMode = Oicana.Config.CompilationMode;
using CompilationOptions = Oicana.Config.CompilationOptions;

namespace Oicana.Test;

public class InputMarshalingLeakTests
{
    [Fact]
    public void RepeatedCompileCallsDoNotLeakNativeInputStringCopies()
    {
        var jsonInputs = new Dictionary<string, JsonNode>
        {
            ["data"] = new JsonObject { ["payload"] = new string('x', 1_000_000) },
        };
        var blobInputs = new Dictionary<string, BlobInput>
        {
            ["blob"] = new BlobInput(new byte[16])
            {
                Metadata = new JsonObject { ["comment"] = new string('y', 1_000_000) },
            },
        };
        var options = new CompilationOptions(CompilationMode.Development);

        var compileOnce = () =>
        {
            try
            {
                OicanaFfi.CompileTemplate("leak-test-unregistered", jsonInputs, blobInputs, options);
            }
            catch (OicanaException)
            {
                // Expected: the template id is not registered. Input marshaling
                // and cleanup still ran in full, which is all this test needs.
            }
        };

        for (var i = 0; i < 10; i++)
        {
            compileOnce();
        }

        var before = MeasurePrivateBytes();

        for (var i = 0; i < 200; i++)
        {
            compileOnce();
        }

        var after = MeasurePrivateBytes();

        // Each iteration marshals ~2 MB of strings to native memory; leaking
        // them accumulates ~400 MB over the loop. The bound leaves generous
        // slack for allocator and GC noise.
        (after - before).Should().BeLessThan(100_000_000);
    }

    private static long MeasurePrivateBytes()
    {
        GC.Collect();
        GC.WaitForPendingFinalizers();
        GC.Collect();
        using var process = Process.GetCurrentProcess();
        process.Refresh();
        return process.PrivateMemorySize64;
    }
}
