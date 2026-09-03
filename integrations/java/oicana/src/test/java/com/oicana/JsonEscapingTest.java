package com.oicana;

import org.junit.jupiter.api.Test;

import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Tests for the JSON serialization of blob metadata.
 */
class JsonEscapingTest {

    /** Metadata with a single entry, so the assertions stay readable. */
    private static String json(Object value) {
        Map<String, Object> metadata = new LinkedHashMap<>();
        metadata.put("k", value);
        return Template.metadataToJson(metadata);
    }

    @Test
    void escapesQuotesAndBackslashes() {
        assertEquals("{\"k\":\"a \\\"quoted\\\" \\\\path\\\\\"}", json("a \"quoted\" \\path\\"));
    }

    @Test
    void escapesNamedControlCharacters() {
        assertEquals("{\"k\":\"\\n\\r\\t\\b\\f\"}", json("\n\r\t\b\f"));
    }

    @Test
    void escapesRemainingControlCharacters() {
        String input = "\0" + (char) 0x01 + (char) 0x1F;
        assertEquals("{\"k\":\"\\u0000\\u0001\\u001f\"}", json(input));
    }

    @Test
    void leavesPlainTextAndNonAsciiUntouched() {
        assertEquals("{\"k\":\"plain text 123\"}", json("plain text 123"));
        assertEquals("{\"k\":\" ~é世\"}", json(" ~é世"));
        assertEquals("{\"k\":\"<a href='b'>&\"}", json("<a href='b'>&"));
    }

    @Test
    void rejectsNonFiniteNumbers() {
        assertThrows(IllegalArgumentException.class, () -> json(Double.NaN));
        assertThrows(IllegalArgumentException.class, () -> json(Double.POSITIVE_INFINITY));
        assertThrows(IllegalArgumentException.class, () -> json(Float.NEGATIVE_INFINITY));
    }

    @Test
    void keepsWholeNumbersWhole() {
        assertEquals("{\"k\":5}", json(5));
        assertEquals("{\"k\":5}", json(5L));
        assertEquals("{\"k\":1.5}", json(1.5));
    }

    @Test
    void serializesPrimitiveArrays() {
        assertEquals("{\"k\":[1,2,3]}", json(new int[] {1, 2, 3}));
        assertEquals("{\"k\":[1.5,2.5]}", json(new double[] {1.5, 2.5}));
        assertEquals("{\"k\":[true,false]}", json(new boolean[] {true, false}));
    }

    @Test
    void serializesObjectArraysAndNestedValues() {
        assertEquals("{\"k\":[\"a\",1,null]}", json(new Object[] {"a", 1, null}));
        assertEquals("{\"k\":[[1,2],[3]]}", json(new int[][] {{1, 2}, {3}}));
        assertEquals("{\"k\":[\"a\",\"b\"]}", json(List.of("a", "b")));
    }

    @Test
    void serializesNestedMaps() {
        Map<String, Object> nested = new LinkedHashMap<>();
        nested.put("image_format", "png");
        nested.put("dpi", 300);
        assertEquals("{\"k\":{\"image_format\":\"png\",\"dpi\":300}}", json(nested));
    }

    @Test
    void keepsNullValues() {
        assertEquals("{\"k\":null}", json(null));
    }

    @Test
    void serializesEmptyMetadata() {
        assertEquals("{}", Template.metadataToJson(Map.of()));
    }
}
