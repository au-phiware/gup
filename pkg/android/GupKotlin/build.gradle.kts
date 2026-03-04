// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "au.com.phiware.gup"
    compileSdk = 34

    defaultConfig {
        minSdk = 24 // Android 7.0 — minimum for Vulkan
        targetSdk = 34

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
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

    // Tell Gradle where to find the pre-built .so files produced by
    // `cargo ndk`.  The CI and example build scripts place them under
    // `jniLibs/{arm64-v8a,armeabi-v7a}/`.
    sourceSets {
        getByName("main") {
            jniLibs.srcDirs("jniLibs")
        }
    }
}
