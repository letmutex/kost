plugins {
    id("org.jetbrains.kotlin.android") version "2.0.20"
}

android {
    compileSdk = 35
}

dependencies {
    implementation(libs.androidx.core.ktx)
    implementation(libs.gson)
    implementation(libs.compose.foundation)
}
