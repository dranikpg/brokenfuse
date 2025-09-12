### Broken Fuse 🔌💥⚡

> How does your application respond to i/o issues? You don't want to mock your i/o layer, it's what you're testing after all. 
> So you stumble upon  _dmsetup_. Loopback device, block device, block size, mke2fs, mount... 😮‍💨 suspend, reload, resume... what block offset is base.db at? 
> AAAHH 😠 dmsetup message x123... Now your CD drive opens. What, heck, you still have that one? SMASH IT 👊 Calm down 😤 Properly mock your filesystem.

Broken fuse is a user-space filesystem built on top of [FUSE](https://www.kernel.org/doc/html/next/filesystems/fuse.html) 
that provides high-level io fault injection. It's not the first of its kind, so what makes it different? It's easy to use:

* Imperative configuration based on [extended attributes](https://wiki.archlinux.org/title/Extended_attributes) applied directly to nodes. 
No sockets, no config files, no regexes, nothing!
* Pythonic wrapper to pull it into intergration tests as a single dependency
* Broad options: fail operations based on probability, on availability time, limit subtree sizes, delay specific operations, compute operation heatmaps... and mix and match everything as you like!

It is intended _exclusively for testing_, never use it for performance and security sensitive cases!

### Quick intro (Python)

Mount it as an in-memory filesystem with the provided context manager.

```py
import brokenfuse as bf
with bf.Fuse('/mnt/fuse'):
  # just a passthrough filesystem
```

"Effects" are applied to all file operations of the subtree they're attached to. Their scope can also be limited by the operation kind - read or write. Let's look at a small example for altering the behaviour of a single file:
```py
f = open('/mnt/fuse/test.txt', 'a') # Create new file

ef_delay = bf.Delay(timedelta(seconds=1), op='r') 
bf.attach(f, ef_delay)                # Delay reads to it by one second
bf.attach(f, bf.Flakey(0.5, op='w'))  # Make 50% of writes fail
some_code_reading_writing(f)          # Observe effects ...

bf.remove(f, ef_delay)        # Remove delay effect
bf.clear('mnt/fuse/test.txt') # Remove all effects. We can also pass a path instead of the file
bf.stats(f)                   # Quickly check stats
```

That's it!

### Quick intro (Binary)

Start the binary and pass the mounth path

```sh
./brokenfuse /mnt/testfs
```

The python wrapper just translates the options to xattr calls with json objects. Repeating the example from above: 

```sh
setfattr test.txt -n bf.effect.delay -v '{"op":"r", "duration_ms":1000}'
setfattr test.txt -n bf.effect.flakey -v '{"op":"w", "prob": 0.5}' 
```

The effect name (the part after `bf.effect.`) determines its type. You can use a hyphen (like this `delay-1`, `delay-two`) to use multiple effects of the same type on the same node.

Deleting an attribute deletes the effect. Deleting `bf.effect` delets all effects.

```sh
setfattr test.txt -x bf.effect.delay
setfattr test.txt -x bf.effect
```

You can also query a files own effects and all effects applied to it (up the tree).

```sh
getfattr test.txt -n bf.effect
getfattr test.txt -n bf.effect/all
```

### Effects

1. Delay `{duration_ms: }`. Delay operations by given number of milliseconds
2. Flakey. Return error based on condition. By default returns errno=5 (Input/output error).
    * `{prob: 0.6, errno: 11}` - return error with 60% prob
    * `{avail: 100, unavail: 200}` - 100ms no errors, 200ms errors in successive intervals
3. Max size `{limit: }`. Limit the subtree size in bytes. Any write spilling over will return ENOSPC.
4. Heatmap `{algin: }`. Build operation heatmap, rounding offset/length to align. Query with getfattr to get data points.
5. Quota `{limit: , align: }` Limit volume of subtree operations, return EDQUOT once exceeded. Round operations up to align.

#### See as well

* https://github.com/ligurio/unreliablefs
* https://github.com/dsrhaslab/lazyfs
* https://github.com/scylladb/charybdefs
