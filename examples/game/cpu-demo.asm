; G16 v1 CPU / keyboard / framebuffer demonstration.
; Import cpu-demo.hex into either game board's Program ROM and Reset to run.
; 16-bit data, r0 always zero, r1-r7 writable, 32-bit instruction words.
; Format: OP[31:27], D[26:24], A[23:21], B[20:18], zero[17:16], IMM[15:0].
; Unused fields must be zero. Two rising clock edges per instruction: fetch, execute.
; LI d,imm; ADD/SUB/AND/OR/XOR/SHL/SHR/SAR/SLTU/SLT d,a,b;
; ADDI/ANDI/ORI/XORI d,a,imm; LD d,base,offset; ST data,base,offset;
; BZ/BNZ a,address; JMP address; JAL link,address; JR a; NOP; HALT.
; Shifts use the low four bits of b. Arithmetic/addressing wrap at 16 bits.
; Program ROM: 64K x 32. Data RAM: 0000-0fff, 4K x 16, cleared on reset.
; Word-addressed IO: ff00 held keys; ff01 event(valid15,pressed3,channel2..0);
; ff02 write acknowledges one event; ff03 write clears keyboard FIFO;
; ff10 X, ff11 Y, ff12 RGB332; ff13 write command(write1,clear2,present4);
; ff20 cycle counter; ff21 random16; ff30 read/write debug output.
; Unmapped reads return zero; unmapped writes do nothing.
.equ EVENT 0xff01
.equ ACK 0xff02
.equ X 0xff10
.equ Y 0xff11
.equ COLOR 0xff12
.equ COMMAND 0xff13
.equ DEBUG 0xff30

start:
    LI r4,0xbeef
    ST r4,r0,0x0010
    LD r5,r0,0x0010
    SUB r4,r5,r4
    BNZ r4,fail
    LI r6,0
    ST r6,r0,COLOR
    LI r5,6
    ST r5,r0,COMMAND
    LI r1,8
    LI r2,8
    JAL r7,draw
    LI r6,0x0038
    ST r6,r0,DEBUG
poll:
    LD r3,r0,EVENT
    BZ r3,poll
    ST r0,r0,ACK
    ANDI r4,r3,8
    BZ r4,poll
    ANDI r3,r3,7
    BZ r3,up
    ADDI r4,r3,-1
    BZ r4,down
    ADDI r4,r3,-2
    BZ r4,left
    ADDI r4,r3,-3
    BZ r4,right
    JMP poll
up:
    ADDI r2,r2,-1
    JMP moved
down:
    ADDI r2,r2,1
    JMP moved
left:
    ADDI r1,r1,-1
    JMP moved
right:
    ADDI r1,r1,1
moved:
    ANDI r1,r1,15
    ANDI r2,r2,15
    JAL r7,draw
    JMP poll
draw:
    ST r1,r0,X
    ST r2,r0,Y
    LI r6,0x001c
    ST r6,r0,COLOR
    LI r5,5
    ST r5,r0,COMMAND
    JR r7
fail:
    LI r6,0xdead
    ST r6,r0,DEBUG
    HALT
