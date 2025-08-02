### Broken Fuse 🔌💥⚡

> You need to test your applications ability to handle i/o issues: 
>   1. _Mock_ your storage layer. At what level? libc? Higher? How do I test i/o code then? 🤦
>   2. _LD_PRELOAD_... Did you know glibc's fopen calls __open? What about io_uring interception? no_std? 🙄
>   3. Stumble upon _dmsetup_. Loopback device, block device, block size, mke2fs, mount... 😮‍💨 suspend, reload, resume... what block offset is base.db at? AAAHH 😠 dmsetup message x123... Now your CD drive opens. What, heck, you still have that one? SMASH IT 👊 Calm down 😤 Proceeed to (4)
>   4. Use Broken Fuse 😎

Broken Fuse is built on top of [FUSE](https://www.kernel.org/doc/html/next/filesystems/fuse.html) (Filesystem in userspace) and provides high-level io fault injection.

It is intended _exclusively for testing_, never use it for performance and security sensitive cases!

#### Quick intro
```sh
./brokenfuse /mnt/testfs
cd /mnt/testfs
echo 'works' > test.txt
cat test.tx
> works
```

Lets verify cat does indeed read our file and not just guess its content.

```sh
getfattr test.txt --only-values -n bk.stats
> {"reads":1,"read_volume":6,"writes":1,"write_volume":6,"errors":0}
```

Let's hold the 🐈‍⬛ back, reads only for now

```sh
setfattr test.txt -n bk.effect.delay -v '{"op":"r","millis":1000}'
time cat test.txt
> 1.0006 total
time (echo 'more text' >> test.txt)
> 0.001 total
```

Effects applied to parent folders are inherited. Let's complicate writing to our filesystem

```sh
setfattr . -n bk.effect.flakey -v '{"op":"w", "prob": 0.5}'      # fail writes with 50% chance in folder
echo 'more text' >> test.txt
> echo: write error: Input/output error
echo 'more text' >> test.txt
> 
```

Cool. 
