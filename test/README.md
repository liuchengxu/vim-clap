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
    1   5.643255             <SNR>37_ext_filter()

====== nvim ======
FUNCTION  <SNR>43_ext_filter()
    1   6.398857             <SNR>43_ext_filter()

total items: 100256
total items: 100256
[multi]
====== vim ======
FUNCTION  <SNR>37_ext_filter()
    3  10.641732             <SNR>37_ext_filter()

====== nvim ======
FUNCTION  <SNR>43_ext_filter()
    3  12.088956             <SNR>43_ext_filter()

[bench100000]
====== vim ======
FUNCTION  <SNR>23_ext_filter()
    1   5.052274             <SNR>23_ext_filter()

====== nvim ======
FUNCTION  <SNR>27_ext_filter()
    1   5.704218             <SNR>27_ext_filter()
```
