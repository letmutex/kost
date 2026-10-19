fun test(args: Array<String>) {
    KtlintCommandLine()
        .subcommands(
            GenerateEditorConfigSubCommand(),
            GitPreCommitHookSubCommand(),
        ).main(args)
}
