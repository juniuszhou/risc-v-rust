# risc-v-rust

## install simulator

```shell
sudo apt install qemu-system-misc
```

## run

```shell
qemu-system-riscv32 -machine virt -nographic -kernel target/riscv32im-unknown-none-elf/release/riscv -bios /usr/lib/riscv64-linux-gnu/opensbi/generic/fw_jump.elf
```

## risc v simulator

## risc v course

## install disassemble tool

```bash
sudo apt install binutils-riscv64-unknown-elf
```

execute

```bash
riscv64-unknown-elf-objdump -d target/riscv32im-unknown-none-elf/release/riscv
```
