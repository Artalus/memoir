`memoir`
---

Memoir is a small tool to monitor RAM usage on a per-process basis. Once started,
it will start collecting a history of RAM usage, and on prompt will dump it to a
tab-separated CSV file:
```csv
Iteration	Timestamp	PID	Name	Memory MB	Command line
1	1705004896927	1210	/usr/bin/i3bar	4	i3bar --bar_id=bar-0
1	1705004896927	362861	/usr/lib/firefox/firefox	80	/usr/lib/firefox/firefox -contentproc -childID 5274 -isForBrowser
2	1705004897930	1210	/usr/bin/i3bar	4	i3bar --bar_id=bar-0
2	1705004897930	362861	/usr/lib/firefox/firefox	91	/usr/lib/firefox/firefox -contentproc -childID 5274 -isForBrowser
...
```

# Building

```bash
$ cargo build --release
```
Cargo will put resulting binary under `target/release/memoirctl`

# Usage

- Use `memoirctl run` to start collecting memory usage.
- You can also use `memoirctl detach` to start Memoir as a background process.
`detach` will exit peacefully upon finding another instance of Memoir, while `run`
will exit with an error code.
- Use `memoirctl save some.csv` to dump collected statistics into a file, and `memoirctl stop`
to stop it.

See [`examples/`](/examples/) directory to see how `memoir` can be used with a build system
or how to interpret its output.
