# Aslet

Aslet is an asynchronous SQLite interface for Godot written in Rust.\
It provides a non-blocking API designed primarily for servers.

## Limitations

- Read-only databases in `res://` cannot be opened in released Godot projects.\
  A common workaround is copying the database from `res://` to `user://` on the
  first run.
- The library is focused on server-side usage, not on client applications.
- It is not possible to create custom SQLite functions from GDScript\
  since all database operations are executed in a separate thread.

## Example

```gdscript
extends Control


var aslet: Aslet


func _init() -> void:
    aslet = Aslet.new()


func _ready() -> void:
    # every async call returns [OK|FAILED, ...]
    var result := await aslet.open('user://users.db').done as Array
    assert(result[0] == OK)

    var db := result[1] as AsletConn
    await db.exec("create table if not exists users (id integer primary key, name text)", []).done
    await db.exec("insert into users (name) values ('Alice')", []).done

    # each inner array represents the parameters for a single row
    var names := [['A'], ['B'], ['C'], ['D'], ['E']]
    result = await db.batch_insert('insert into users (name) values (?1)', names).done
    assert(result[0] == OK)

    # the third value of the return from fetch is a PackedStringArray
    # containing all queried column names
    result = await db.fetch("select * from users", []).done
    assert(result[2] == PackedStringArray(['id', 'name']))
    var rows := result[1] as Array
    for row in rows:
        print(row[0], row[1])

    # transactions are independent/isolated
    var tx = (await db.transaction().done)[1] as AsletTransaction
    await tx.exec('insert into users (name) value (?1)', ['hello world']).done
    await tx.commit().done

    # once committed or rolled back, a transaction becomes invalid
    result = await tx.exec('insert into users (name) values (?1)', ['hello world again']).done
    assert(result[0] == FAILED)

    # incremental backup with progress callback
    # copies the database in chunks (10 pages per tick)
    var on_tick := func(page_count: int, remaining: int):
        print('tick %d / %d' % [page_count - remaining, page_count])
    result = await db.backup('user://users2.db', 10, on_tick).done
    assert(result[0] == OK)


func _process(_dt: float) -> void:
    # small timeout means less impact on the main thread and slower task handling
    aslet.poll(5)
```
