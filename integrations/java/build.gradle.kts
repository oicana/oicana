buildscript {
    repositories {
        mavenCentral()
        gradlePluginPortal()
    }
    dependencies {
        classpath("com.vanniktech:gradle-maven-publish-plugin:0.36.0")
    }
}

subprojects {
    group = "com.oicana"
    version = "0.9.0-rc.2"

    apply(plugin = "java-library")
    apply(plugin = "com.vanniktech.maven.publish")

    configure<JavaPluginExtension> {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    if (project.name != "oicana") {
        tasks.named<Jar>("jar") {
            manifest {
                attributes(
                    "Automatic-Module-Name" to
                        "com.oicana.natives.${project.name.removePrefix("oicana-").replace('-', '.')}"
                )
            }
        }
    }

    repositories {
        mavenCentral()
    }

    afterEvaluate {
        extensions.configure<com.vanniktech.maven.publish.MavenPublishBaseExtension> {
            pom {
                name.set(project.name)
                description.set(
                    if (project.name == "oicana") {
                        "Generate typeset PDFs on the JVM from Typst templates, in process. No headless browser, no per-document fees."
                    } else {
                        "Native library for Oicana PDF generation (${project.name.removePrefix("oicana-")})."
                    }
                )
                url.set("https://oicana.com")
                licenses {
                    license {
                        name.set("PolyForm Noncommercial License 1.0.0")
                        url.set("https://polyformproject.org/licenses/noncommercial/1.0.0/")
                    }
                }
                developers {
                    developer {
                        id.set("oicana")
                        name.set("Oicana")
                        email.set("hello@oicana.com")
                        organization.set("Oicana")
                        organizationUrl.set("https://oicana.com")
                    }
                }
                scm {
                    connection.set("scm:git:git://github.com/oicana/oicana.git")
                    developerConnection.set("scm:git:ssh://github.com:oicana/oicana.git")
                    url.set("https://github.com/oicana/oicana")
                }
            }
        }
    }
}
