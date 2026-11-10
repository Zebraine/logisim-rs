; Snake / G16 v1. No game-specific hardware or host callbacks.
; Open game.circ, enable Auto ticks at 4096 Hz, Poke the keyboard.
; Space starts/pauses; arrows turn; Enter restarts; Escape releases focus.
; White head, green body, red food. Walls/self end the game; fill all 256 cells to win.
; At most one perpendicular turn per move; extra turns in that interval are ignored.
; DEBUG = state*0x1000 + score: state 0 running, 1 paused, 2 lost, 3 won.
; ISA/MMIO reference: cpu-demo.asm. All addresses below are 16-bit word addresses.
; RAM: variables 0..31; body ring at 0100; occupancy map at 0200.
; pixel(r1=packed YX,r2=RGB332) clobbers r3/r4, preserves r5/r6; r7 link.
.equ EVENT 0xff01
.equ ACK 0xff02
.equ KEY_CLEAR 0xff03
.equ X 0xff10
.equ Y 0xff11
.equ COLOR 0xff12
.equ COMMAND 0xff13
.equ TIMER 0xff20
.equ RANDOM 0xff21
.equ DEBUG 0xff30
.equ LEN 0
.equ TAIL 1
.equ HEAD 2
.equ DIR 3
.equ NEXTDIR 4
.equ TURNED 5
.equ FOOD 6
.equ STATE 7
.equ LAST 8
.equ MOVES 9
.equ READY 10
.equ GROW 12
.equ NEWPOS 13
.equ TAILPOS 14
.equ FRAME 16
.equ BODY 0x0100
.equ OCC 0x0200
.equ SPEED 400

start:
    ST r0,r0,READY
    ST r0,r0,KEY_CLEAR
    ST r0,r0,COLOR
    LI r1,2
    ST r1,r0,COMMAND
    LI r1,256
clear_map:
    ADDI r1,r1,-1
    ST r0,r1,OCC
    BNZ r1,clear_map
    ST r0,r0,TAIL
    ST r0,r0,DIR
    ST r0,r0,NEXTDIR
    ST r0,r0,TURNED
    ST r0,r0,MOVES
    ST r0,r0,FRAME
    LI r1,3
    ST r1,r0,LEN
    LI r1,1
    ST r1,r0,STATE
    LI r5,0
    LI r1,0x85
    LI r2,28
initial_body:
    ST r1,r5,BODY
    LI r3,1
    ST r3,r1,OCC
    JAL r7,pixel
    ADDI r1,r1,1
    ADDI r5,r5,1
    LI r3,3
    SUB r3,r5,r3
    BNZ r3,initial_body
    LI r1,0x87
    ST r1,r0,HEAD
    LI r2,255
    JAL r7,pixel
    LI r1,0x8a
    ST r1,r0,FOOD
    LI r2,224
    JAL r7,pixel
    LI r1,1
    ST r1,r0,READY
    JMP frame_ready

main_loop:
    LD r1,r0,EVENT
    BZ r1,time_check
    ST r0,r0,ACK
    ANDI r2,r1,8
    BZ r2,time_check
    ANDI r1,r1,7
    ADDI r2,r1,-5
    BZ r2,start
    ADDI r2,r1,-4
    BZ r2,toggle_pause
    LD r2,r0,STATE
    BNZ r2,time_check
    LD r2,r0,TURNED
    BNZ r2,time_check
    LI r2,4
    SLTU r2,r1,r2
    BZ r2,time_check
    ; channel 0/1/2/3 -> direction 3/1/2/0
    ADD r3,r1,r1
    LI r2,39
    SHR r2,r2,r3
    ANDI r2,r2,3
    LD r3,r0,DIR
    XORI r4,r3,2
    SUB r4,r4,r2
    BZ r4,time_check
    SUB r4,r3,r2
    BZ r4,time_check
    ST r2,r0,NEXTDIR
    LI r2,1
    ST r2,r0,TURNED
    JMP time_check
toggle_pause:
    LD r1,r0,STATE
    LI r2,2
    SLTU r2,r1,r2
    BZ r2,time_check
    XORI r1,r1,1
    ST r1,r0,STATE
    LD r1,r0,TIMER
    ST r1,r0,LAST
    JAL r7,status
    JMP main_loop
time_check:
    LD r1,r0,STATE
    BNZ r1,main_loop
    LD r1,r0,TIMER
    LD r2,r0,LAST
    SUB r1,r1,r2
    LI r2,SPEED
    SLTU r1,r1,r2
    BNZ r1,main_loop
move:
    LD r1,r0,HEAD
    LD r2,r0,NEXTDIR
    ST r2,r0,DIR
    BZ r2,right
    ADDI r3,r2,-1
    BZ r3,down
    ADDI r3,r2,-2
    BZ r3,left
    LI r3,16
    SLTU r3,r1,r3
    BNZ r3,lost
    ADDI r1,r1,-16
    JMP candidate
right:
    ANDI r3,r1,15
    ADDI r3,r3,-15
    BZ r3,lost
    ADDI r1,r1,1
    JMP candidate
down:
    LI r3,240
    SLTU r3,r1,r3
    BZ r3,lost
    ADDI r1,r1,16
    JMP candidate
left:
    ANDI r3,r1,15
    BZ r3,lost
    ADDI r1,r1,-1
candidate:
    ST r1,r0,NEWPOS
    ST r0,r0,GROW
    LD r2,r0,FOOD
    SUB r2,r1,r2
    BNZ r2,check_body
    LI r2,1
    ST r2,r0,GROW
check_body:
    LD r3,r0,TAIL
    LD r4,r3,BODY
    ST r4,r0,TAILPOS
    LD r2,r1,OCC
    BZ r2,commit_move
    LD r2,r0,GROW
    BNZ r2,lost
    SUB r2,r1,r4
    BNZ r2,lost
commit_move:
    LD r2,r0,GROW
    BNZ r2,grow
    LD r1,r0,TAILPOS
    ST r0,r1,OCC
    LI r2,0
    JAL r7,pixel
    LD r3,r0,TAIL
    ADDI r3,r3,1
    ANDI r3,r3,255
    ST r3,r0,TAIL
    JMP add_head
grow:
    LD r3,r0,LEN
    ADDI r3,r3,1
    ST r3,r0,LEN
add_head:
    LD r1,r0,HEAD
    LI r2,28
    JAL r7,pixel
    LD r1,r0,NEWPOS
    ST r1,r0,HEAD
    LD r3,r0,TAIL
    LD r4,r0,LEN
    ADD r3,r3,r4
    ADDI r3,r3,-1
    ANDI r3,r3,255
    ST r1,r3,BODY
    LI r3,1
    ST r3,r1,OCC
    LI r2,255
    JAL r7,pixel
    LD r1,r0,MOVES
    ADDI r1,r1,1
    ST r1,r0,MOVES
    LD r1,r0,GROW
    BZ r1,frame_ready
    LD r1,r0,LEN
    ADDI r1,r1,-256
    BZ r1,won
    ; One random candidate then a bounded wraparound scan of all cells.
    LD r1,r0,RANDOM
    ANDI r1,r1,255
    LI r5,256
find_food:
    LD r2,r1,OCC
    BZ r2,food_found
    ADDI r1,r1,1
    ANDI r1,r1,255
    ADDI r5,r5,-1
    BNZ r5,find_food
    JMP won
food_found:
    ST r1,r0,FOOD
    LI r2,224
    JAL r7,pixel
    JMP frame_ready
lost:
    LI r1,2
    ST r1,r0,STATE
    LD r1,r0,HEAD
    LI r2,224
    JAL r7,pixel
    JMP frame_ready
won:
    LI r1,3
    ST r1,r0,STATE
    LI r1,28
    ST r1,r0,COLOR
    LI r1,2
    ST r1,r0,COMMAND
frame_ready:
    LI r1,4
    ST r1,r0,COMMAND
    ST r0,r0,TURNED
    LD r1,r0,FRAME
    ADDI r1,r1,1
    ST r1,r0,FRAME
    LD r1,r0,TIMER
    ST r1,r0,LAST
    JAL r7,status
    JMP main_loop
status:
    LD r1,r0,STATE
    LI r2,12
    SHL r1,r1,r2
    LD r2,r0,LEN
    ADDI r2,r2,-3
    OR r1,r1,r2
    ST r1,r0,DEBUG
    JR r7
pixel:
    ANDI r3,r1,15
    ST r3,r0,X
    LI r3,4
    SHR r3,r1,r3
    ST r3,r0,Y
    ST r2,r0,COLOR
    LI r4,1
    ST r4,r0,COMMAND
    JR r7
