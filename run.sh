qemu-system-x86_64 -bios /usr/share/edk2/x64/OVMF.4m.fd -drive format=raw,file=target/axiom_mkern.img,if=ide,index=0 -drive format=raw,file=target/axiom_data.img,if=ide,index=1 -m 512M
