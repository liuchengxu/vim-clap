# Quick Start

## Profile fzy implementation in Pure Python and Rust

```bash
$ cd bench/python
```

```bash
$ cd test
$ bash fetch_testdata.sh
$ ./run-profile.sh --all
```

- OS: macOS 10.14.6
- Machine: MBP 18 15-inch, 2.2GHz Intel Core i7, 32 GB 2400 MHz DDR4.

<table style="width: 100%;">

<tr><th>Pure Python</th><th>Rust</th></tr>
<tr>

<td>

```
stats of pure Python fuzzy filter performance:

total items: 100257
[once]
====== vim ======
FUNCTION  <SNR>37_ext_filter()
1   5.951908             <SNR>37_ext_filter()

====== nvim ======
FUNCTION  <SNR>43_ext_filter()
1   6.592639             <SNR>43_ext_filter()

total items: 100257
total items: 100257
[multi]
====== vim ======
FUNCTION  <SNR>37_ext_filter()
3  11.148641             <SNR>37_ext_filter()

====== nvim ======
FUNCTION  <SNR>43_ext_filter()
3  12.655566             <SNR>43_ext_filter()

[bench100000]
====== vim ======
FUNCTION  <SNR>23_ext_filter()
1   5.400793             <SNR>23_ext_filter()

====== nvim ======
FUNCTION  <SNR>27_ext_filter()
1   5.921336             <SNR>27_ext_filter()
```

</td>

<td>

```
stats of Rust fuzzy filter performance:

total items: 100257
total items: 100257
[once]
====== vim ======
FUNCTION  <SNR>37_ext_filter()
1   0.352795             <SNR>37_ext_filter()

====== nvim ======
FUNCTION  <SNR>43_ext_filter()
1   0.940487             <SNR>43_ext_filter()

total items: 100257
total items: 100257
[multi]
====== vim ======
FUNCTION  <SNR>37_ext_filter()
3   0.843340             <SNR>37_ext_filter()

====== nvim ======
FUNCTION  <SNR>43_ext_filter()
3   2.221188             <SNR>43_ext_filter()

[bench100000]
====== vim ======
FUNCTION  <SNR>23_ext_filter()
1   0.306407             <SNR>23_ext_filter()

====== nvim ======
FUNCTION  <SNR>27_ext_filter()
1   0.865437             <SNR>27_ext_filter()
```

</td>

</tr>
</table>

### Conclusion

- `pynvim` of Neovim is slower than `python` of vim, especially when using the Python dynamic module written in Rust. The bottlenecks in this case is the `roundtrips` of neovim, see https://github.com/liuchengxu/vim-clap/commit/75eb77ec5b6c0263cbfb55a90865234a3eafbf3d#r36451422 .
- With Rust, vim is practically 16x faster, neovim is 7x faster.
