val shouldUseAndroidTheme: Boolean = when (userData.themeBrand) {
    ThemeBrand.DEFAULT -> false
    ThemeBrand.ANDROID -> true
}
