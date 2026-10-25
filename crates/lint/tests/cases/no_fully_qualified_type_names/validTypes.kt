package com.example.demo

import com.example.User
import java.util.Map

val a: String = "test"
val b: Int = 1
val c: User = User()
val d: User? = null
val list: List<String> = emptyList()
val entry: Map.Entry<String, Int>? = null
val state: Result.Success? = null

fun process(user: User): String = user.name

class Service : BaseService(), CleanInterface

typealias UserMap = Map<String, User>

val res = com.example.Util.doWork()

fun checkTypes(obj: Any) {
    val s = obj as String
    val check = obj is String
}

fun User.fullName(): String = "$firstName $lastName"

class Box<T : Comparable<T>>

val fn: (Int, String) -> Boolean = { _, _ -> true }
val nestedClean: Outer<String>.Inner = TODO()


@Suppress("ktlint:standard:no-fully-qualified-type-names")
val suppressedFqn: com.example.Suppressed = TODO()

@Suppress("ktlint:no-fully-qualified-type-names")
val suppressedWithoutStandard: com.example.SuppressedTwo = TODO()

