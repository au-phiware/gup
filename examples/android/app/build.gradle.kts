// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "au.com.phiware.gup.example"
    compileSdk = 34

    defaultConfig {
        applicationId = "au.com.phiware.gup.example"
        minSdk = 24
        targetSdk = 34
        versionCode = 1
        versionName = "0.1.0"
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    // Pre-built native libraries from `cargo ndk`
    sourceSets {
        getByName("main") {
            jniLibs.srcDirs("src/main/jniLibs")
        }
    }
}

dependencies {
    // In a real project you would depend on the GupKotlin .aar.
    // For the example we compile the Kotlin sources directly.
    implementation(fileTree(mapOf("dir" to "../../../pkg/android/GupKotlin/src/main/java", "include" to listOf("**/*.kt"))))
}
