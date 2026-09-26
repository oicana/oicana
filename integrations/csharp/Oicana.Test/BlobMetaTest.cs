using System.Text.Json.Nodes;
using AwesomeAssertions;
using Oicana.Inputs;

namespace Oicana.Test;

public class BlobMetaTest
{
    [Fact]
    public void CombinesObjectWithImageFormat()
    {
        var meta = new BlobMeta()
        {
            Custom = JsonNode.Parse(
                """
                {
                   "bar": ["input", "two"],
                   "foo": 42
                }
                """)!.AsObject(),
            ImageFormat = "png"
        };

        meta.Build().ToString().Should().Be(
            JsonNode.Parse(
                """
                {
                   "bar": ["input", "two"],
                   "foo": 42,
                   "image_format": "png"
                }
                """)!.AsObject().ToString());
    }

    [Fact]
    public void ImageFormatDoesNotGetSetIfItClashesWithCustomObject()
    {
        var meta = new BlobMeta()
        {
            Custom = JsonNode.Parse(
                """
                {
                   "bar": ["input", "two"],
                   "foo": 42,
                   "image_format": "jpeg"
                }
                """)!.AsObject(),
            ImageFormat = "png"
        };

        meta.Build().ToString().Should().Be(
            JsonNode.Parse(
                """
                {
                   "bar": ["input", "two"],
                   "foo": 42,
                   "image_format": "jpeg"
                }
                """)!.AsObject().ToString());
    }

    [Fact]
    public void OnlyCustom()
    {
        var meta = new BlobMeta()
        {
            Custom = JsonNode.Parse(
                """
                {
                   "bar": ["input", "two"],
                   "foo": 42
                }
                """)!.AsObject()
        };

        meta.Build().ToString().Should().Be(
            JsonNode.Parse(
                """
                {
                   "bar": ["input", "two"],
                   "foo": 42
                }
                """)!.AsObject().ToString());
    }

    [Fact]
    public void OnlyImageFormat()
    {
        var meta = new BlobMeta()
        {
            ImageFormat = "png"
        };

        meta.Build().ToString().Should().Be(
            JsonNode.Parse(
                """
                {
                   "image_format": "png"
                }
                """)!.AsObject().ToString());
    }

    [Fact]
    public void EmptyMeta()
    {
        var meta = new BlobMeta();

        meta.Build().ToString().Should().Be(
            JsonNode.Parse("{}")!.AsObject().ToString());
    }

    [Fact]
    public void BuildLeavesTheCustomObjectUntouched()
    {
        var custom = new JsonObject { ["dpi"] = 300 };
        var png = new BlobMeta { ImageFormat = "png", Custom = custom };
        var jpg = new BlobMeta { ImageFormat = "jpg", Custom = custom };

        png.Build()["image_format"]!.GetValue<string>().Should().Be("png");
        jpg.Build()["image_format"]!.GetValue<string>().Should().Be("jpg");
        custom.ContainsKey("image_format").Should().BeFalse();
    }
}
