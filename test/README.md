# Quick Start

## Profile fzy implementation in Pure Python and Rust

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
stats of fuzzy filter performance:

total items: 100256
[once]
====== vim ======
FUNCTION  <SNR>37_ext_filter()
    1   5.739779             <SNR>37_ext_filter()

====== nvim ======
FUNCTION  <SNR>43_ext_filter()
    1   6.427331             <SNR>43_ext_filter()

total items: 100256
total items: 100256
[multi]
====== vim ======
FUNCTION  <SNR>37_ext_filter()
    3  10.802956             <SNR>37_ext_filter()

====== nvim ======
FUNCTION  <SNR>43_ext_filter()
    3  12.251384             <SNR>43_ext_filter()

[bench100000]
====== vim ======
FUNCTION  <SNR>23_ext_filter()
    1   5.228887             <SNR>23_ext_filter()

====== nvim ======
FUNCTION  <SNR>27_ext_filter()
    1   5.793226             <SNR>27_ext_filter()
```

</td>

<td>

```
stats of fuzzy filter performance:

total items: 100256
total items: 100256
[once]
====== vim ======
FUNCTION  <SNR>37_ext_filter()
    1   0.352634             <SNR>37_ext_filter()

====== nvim ======
FUNCTION  <SNR>43_ext_filter()
    1   0.933202             <SNR>43_ext_filter()

total items: 100256
total items: 100256
[multi]
====== vim ======
FUNCTION  <SNR>37_ext_filter()
    3   0.860233             <SNR>37_ext_filter()

====== nvim ======
FUNCTION  <SNR>43_ext_filter()
    3   2.216256             <SNR>43_ext_filter()

[bench100000]
====== vim ======
FUNCTION  <SNR>23_ext_filter()
    1   0.301782             <SNR>23_ext_filter()

====== nvim ======
FUNCTION  <SNR>27_ext_filter()
    1   0.848937             <SNR>27_ext_filter()
```

</td>

</tr>
</table>

### Conclusion

- `pynvim` of Neovim is slower `python` of vim, especially when using the Python dynamic module.
- With Rust, vim is practically 16x faster, neovim is 7x faster.
