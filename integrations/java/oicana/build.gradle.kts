dependencies {
    implementation("com.google.code.gson:gson:2.11.0")

    testImplementation("org.junit.jupiter:junit-jupiter:5.11.4")
    testRuntimeOnly("org.junit.platform:junit-platform-launcher")
    testRuntimeOnly(project(":oicana-linux-x86_64"))
    testRuntimeOnly(project(":oicana-linux-aarch64"))
    testRuntimeOnly(project(":oicana-macos-x86_64"))
    testRuntimeOnly(project(":oicana-macos-aarch64"))
    testRuntimeOnly(project(":oicana-windows-x86_64"))
}

tasks.test {
    useJUnitPlatform()
}

tasks.jar {
    from("README.md")
}
