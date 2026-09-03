/**
 * Oicana PDF templating engine for Java.
 */
module com.oicana {
    requires com.google.gson;

    exports com.oicana;
    exports com.oicana.manifest;

    opens com.oicana.manifest to com.google.gson;
}
