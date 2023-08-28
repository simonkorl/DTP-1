
# DTP 基础功能测试说明

在tests目录下有两个文件：

1. crate_bigfile.py: 用于创建大文件，用于传输大文件
2. base_function.rs: 用于测试DTP的基础功能

## crate_bigfile.py使用方法

`python crate_bigfile.py <filePath> <fileSize>` 
e.g. `python crate_bigfile.py ./1G.txt 1G`

## base_function.rs使用方法

首先使用Python脚本创建一个大文件，用于测试文件传输功能。然后
在tests目录下分别执行：
`cargo test test_transport_file`
`cargo test test_transport_order`
**不能直接执行cargo test。log库初始化会报错，另外会导致端口占用无法进行两个测试**

1. test_transport_file: 测试文件传输功能,修改测试函数中的文件路径，用来传输文件。结果会在result中生成。可以通过diff命令来比较文件是否一致。`diff <file1> <file2>`如过没有输出，则说明文件一致。
2. test_transport_order: 测试顺序传输功能。它会发送两个字符串，分别设置了不同的优先级。在result中生成的文件中应该看到"World Hello"的顺序。
