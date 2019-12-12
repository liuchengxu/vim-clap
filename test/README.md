# Quick Start

## Profile fzy implementation in Python and Rust

### Pure python

```bash
$ ./run-profile.sh --all
```

macOS:

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
