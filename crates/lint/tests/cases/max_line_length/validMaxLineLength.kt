val shortLine = 42
val unicodeString = "这是五十个汉字的测试文本用于测试行长度规则正确按字符计数而不是按字节计数计算并且不会报错测试结束啦"

/**
 * This is a very long line inside a KDoc block comment that definitely exceeds the one hundred and forty character limit configured in max-line-length rule but should be excluded from reporting because KDoc comments are exempt.
 */
