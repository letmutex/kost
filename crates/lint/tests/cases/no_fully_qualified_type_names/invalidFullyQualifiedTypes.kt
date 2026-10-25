package com.example.demo

class ServiceA : com.example.BaseClass()
class ServiceB : com.example.BaseInterface

val user: com.example.User = TODO()
var count: java.util.concurrent.atomic.AtomicInteger = TODO()

fun process(param: com.example.Param): com.example.Result = TODO()

typealias UserList = java.util.List<com.example.User>

val items: List<com.example.Item> = emptyList()

class Container<T : com.example.Bound>

fun castAndCheck(obj: Any) {
    val a = obj as com.example.Target
    val b = obj is com.example.Check
}

fun com.example.Receiver.ext(): String = ""

val callback: (com.example.In) -> com.example.Out = TODO()

val nestedParam: com.example.Outer<String>.Inner = TODO()
val unicodePkg: élan.company.MyClass = TODO()

