#!/bin/env python3

def add(src, dest):
    print('%s\t%s' % (src, dest))

add('src/a.txt', 'dest/a.txt')
add('src/b.txt', 'dest/b.txt')

print('# Test comment')
add('src/c.txt', 'dest/c.txt')
