
import os
import sys
import random
import string

# generate random string with size


def random_str(size):
    return ''.join(random.choice(string.ascii_letters) for _ in range(size))


# generate big file with random content


def create_bigfile(filename, size):
    (line_size, line_num, bytes_gap) = calulate_size(size)
    # print(line_size, line_num, bytes_gap)
    line_size = line_size - 1
    line1 = random_str(line_size)
    line2 = random_str(line_size)
    with open(filename, 'w') as f:
        for i in range(line_num):
            if i % 2 == 0:
                f.write(line1+'\n')
            else:
                f.write(line2+'\n')
        if bytes_gap-1 > 0:
            f.write(random_str(bytes_gap)+'\n')
        else:
            if bytes_gap == 1:
                f.write('\n')
    f.close()


# calculate size needed line number and line size


def calulate_size(size):
    if size < 1024:
        line_size = size
        line_num = 1
        gap = 0
    else:
        if size >= 1024 * 1024:
            line_size = 1024 * 1024
            line_num = int(size / line_size)
            gap = size - line_size * line_num
        else:
            line_size = 1024
            line_num = int(size / line_size)
            gap = size - line_size * line_num
    return line_size, line_num, gap

# parse user input size, transform to byte


def parse_size_type(size_str):
    size_str = size_str.upper()
    if size_str[-1] == 'K':
        return int(size_str[:-1]) * 1024
    elif size_str[-1] == 'M':
        return int(size_str[:-1]) * 1024 * 1024
    elif size_str[-1] == 'G':
        return int(size_str[:-1]) * 1024 * 1024 * 1024
    elif size_str[-1] == 'T':
        return int(size_str[:-1]) * 1024 * 1024 * 1024 * 1024
    else:
        return int(size_str)


if __name__ == '__main__':
    if len(sys.argv) < 3:
        print('Usage: python create_bigfile.py <filename> <size>, e.g. 1K, 1M, 1G, 1T,default size type is byte')
        sys.exit(1)
    filename = sys.argv[1]
    size = parse_size_type(sys.argv[2])
    create_bigfile(filename, size)
    print('File %s created with size %d' % (filename, size))
