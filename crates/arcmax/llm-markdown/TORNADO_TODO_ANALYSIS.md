# Tornado Compression Algorithm - Developer To-Do List Analysis

## Translation and Summary of Key Points from to-do.txt

This document analyzes the original developer's to-do list for the Tornado compression algorithm, highlighting key improvements and optimizations they planned.

## Major Improvements Planned

### 1. Hash Function Optimizations
- **Better hash multiplier**: Use 0x65a8e9b4 for hash function
- **Store unused hash bits**: Store unused hash bits + additional characters in hash for 1.5x speedup
- **Improved caching**: Save more data in hash records for faster matching
- **Separate cycles for different lengths**: Separate loops for len=3,4,5,6 for better performance

### 2. Match Finding Optimizations
- **Lazy search**: Implement lazy search for highest compression mode
- **Skip short distant matches**: Skip short matches that are far away to improve compression
- **Better hash update**: Full hash update for highest modes
- **3-byte string support**: Add support for 3-byte strings

### 3. Compression Algorithm Improvements
- **Combined len+dist encoding**: Like CABARC for faster decoding
- **Multiple encoding tables**: Context-based tables (after null chars, after small chars)
- **Diffing tables**: Support for data table differencing
- **Context-based character encoding**: Separate coder tables after null or control characters

### 4. Performance Optimizations
- **Fast arithmetic**: Optimize arithmetic operations with total=2^n
- **Bit I/O improvements**: Separate buffer for bit field reading
- **Optimized lazy matching**: Various techniques to improve lazy match performance
- **Sliding window optimizations**: Different window sizes for different compression levels

### 5. Advanced Features
- **Optimal parsing**: Full optimal parsing implementation with multiple matches
- **REP* codes**: Support for repdist, repboth, repchar codes
- **ROLZ support**: Implement ROLZ 1+2+3+4 algorithms
- **Multithreading**: Support for multiple blocks and parallel processing

### 6. Memory Management
- **Auto-decrease hash**: Automatically decrease hash and buffer size for small files
- **Better memory allocation**: Optimize memory usage for different file sizes
- **Cyclic hash**: For large N values

### 7. Format Improvements
- **Header improvements**: Better header format with signature, version, flags, CRC
- **Block-based output**: Output data in chunks corresponding to input chunks
- **Progress indicators**: Console title updates and progress reporting

## Technical Insights from the Developer

### Performance Techniques Mentioned:
1. **Lasse Reinhold's optimization**: Cache bytes 3-6 for faster match comparison among N possibilities
2. **Kadach huffman**: Advanced Huffman coding techniques
3. **Prefetching**: In CachingMatchFinder for better performance
4. **Bit caching**: BitCachingMatchFinder similar to HT4 using upper bits

### Compression Ratio Improvements:
1. **Lazy matching**: +3.5% compression improvement mentioned
2. **Arithmetic/Huffman coding**: +10% compression for bit I/O, +20% for Huffman
3. **REP* codes**: Better handling of repeated distances and characters
4. **Optimal parsing**: Significant compression improvements with full match evaluation

## What the Developer Thinks About Tornado

Based on the to-do list, the original developer viewed Tornado as:
- A high-performance LZ77-based compressor with focus on speed
- An algorithm that could benefit significantly from advanced parsing techniques
- A format that needed better integration with modern compression concepts (REP* codes, optimal parsing)
- A system that could achieve better compression ratios with proper optimizations

## Potential Improvements We Can Implement

### 1. Safe Improvements (Won't Break Compatibility)
- **Better hash functions**: Using improved hash multipliers like 0x65a8e9b4
- **Lazy matching**: Implementing lazy search for better compression
- **REP* code support**: Adding repdist, repchar, repboth codes
- **Optimized match finding**: Better algorithms for finding and evaluating matches

### 2. Performance Enhancements
- **Faster arithmetic**: Optimized arithmetic coding operations
- **Memory optimizations**: Better hash table usage and memory management
- **Sliding window**: More efficient window management for different modes

### 3. Compression Quality Improvements
- **Optimal parsing**: Full optimal parsing instead of greedy/lazy
- **Context modeling**: Better context-based probability estimation
- **Multiple encoders**: Dynamic selection between byte/bit/huffman/arithmetic coders

## Implementation Strategy

For FreeARC integration, we should focus on:
1. **Safe optimizations** that maintain format compatibility
2. **Performance improvements** that don't change the compression format
3. **Bug fixes** mentioned in the to-do list
4. **REP* code enhancements** for better compression of repetitive data

The original developer had ambitious plans for Tornado, including optimal parsing and advanced context modeling, which could significantly improve both compression ratio and speed while maintaining backward compatibility.