; Two-player Ping Pong / G16 v1. Same CPU and peripherals as game.circ.
; Open ping-pong.circ, enable Auto ticks at 4096 Hz, Poke the keyboard.
; Space starts/pauses; Up/Down moves the green left paddle; Z/X moves blue right.
; Hold a key to move; opposite keys together stop that paddle. Enter restarts.
; First to seven points wins. Top row shows scores; the winning ball takes player color.
; DEBUG = state*0x1000 + left_score*0x100 + right_score.
; state: 0 playing, 1 paused, 2 left won, 3 right won. Escape releases focus.
; Field y=2..15; four-pixel paddles at x=0/15; ball advances every second frame.
; ISA/MMIO reference: cpu-demo.asm. RAM variables 0..15 are software state only.
.equ HELD 0xff00
.equ EVENT 0xff01
.equ ACK 0xff02
.equ KEY_CLEAR 0xff03
.equ X 0xff10
.equ Y 0xff11
.equ COLOR 0xff12
.equ COMMAND 0xff13
.equ TIMER 0xff20
.equ DEBUG 0xff30
.equ LEFT 0
.equ RIGHT 1
.equ BX 2
.equ BY 3
.equ DX 4
.equ DY 5
.equ LSCORE 6
.equ RSCORE 7
.equ STATE 8
.equ LAST 9
.equ READY 10
.equ FRAME 11
.equ DIVIDER 12
.equ SPEED 160

start:
    ST r0,r0,READY
    ST r0,r0,KEY_CLEAR
    ST r0,r0,LSCORE
    ST r0,r0,RSCORE
    ST r0,r0,FRAME
    ST r0,r0,DIVIDER
    LI r1,6
    ST r1,r0,LEFT
    ST r1,r0,RIGHT
    LI r1,1
    ST r1,r0,STATE
    ST r1,r0,DX
    ST r1,r0,DY
    LI r1,8
    ST r1,r0,BX
    ST r1,r0,BY
    LI r1,1
    ST r1,r0,READY
    JMP render
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
    BNZ r2,time_check
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
update:
    LD r1,r0,LEFT
    LD r2,r0,HELD
    JAL r7,paddle
    ST r1,r0,LEFT
    LD r1,r0,RIGHT
    LD r2,r0,HELD
    LI r3,6
    SHR r2,r2,r3
    JAL r7,paddle
    ST r1,r0,RIGHT
    LD r1,r0,DIVIDER
    XORI r1,r1,1
    ST r1,r0,DIVIDER
    BNZ r1,render
    ; Reflect vertically before testing a paddle at the next Y position.
    LD r1,r0,BX
    LD r2,r0,BY
    LD r3,r0,DX
    LD r4,r0,DY
    ADD r2,r2,r4
    LI r5,2
    SLTU r5,r2,r5
    BNZ r5,reflect_y
    LI r5,16
    SLTU r5,r2,r5
    BNZ r5,advance_x
reflect_y:
    SUB r4,r0,r4
    ST r4,r0,DY
    LD r2,r0,BY
    ADD r2,r2,r4
advance_x:
    ADD r1,r1,r3
    BZ r1,left_edge
    ADDI r5,r1,-15
    BZ r5,right_edge
    JMP store_ball
left_edge:
    LD r5,r0,LEFT
    SLTU r6,r2,r5
    BNZ r6,right_point
    SUB r6,r2,r5
    LI r5,4
    SLTU r5,r6,r5
    BZ r5,right_point
    LI r3,1
    LI r1,1
    JMP paddle_bounce
right_edge:
    LD r5,r0,RIGHT
    SLTU r6,r2,r5
    BNZ r6,left_point
    SUB r6,r2,r5
    LI r5,4
    SLTU r5,r6,r5
    BZ r5,left_point
    LI r3,-1
    LI r1,14
paddle_bounce:
    ST r3,r0,DX
    LI r4,1
    LI r5,2
    SLTU r5,r6,r5
    BZ r5,bounce_done
    LI r4,-1
bounce_done:
    ST r4,r0,DY
store_ball:
    ST r1,r0,BX
    ST r2,r0,BY
    JMP render
left_point:
    LD r1,r0,LSCORE
    ADDI r1,r1,1
    ST r1,r0,LSCORE
    LI r3,1
    ST r3,r0,DX
    ADDI r2,r1,-7
    BNZ r2,serve
    LI r1,2
    ST r1,r0,STATE
    JMP serve
right_point:
    LD r1,r0,RSCORE
    ADDI r1,r1,1
    ST r1,r0,RSCORE
    LI r3,-1
    ST r3,r0,DX
    ADDI r2,r1,-7
    BNZ r2,serve
    LI r1,3
    ST r1,r0,STATE
serve:
    LI r1,8
    ST r1,r0,BX
    ST r1,r0,BY
    LI r1,1
    ST r1,r0,DY
    ST r0,r0,DIVIDER
render:
    ; Draw into the back buffer, then publish one complete frame.
    ST r0,r0,COLOR
    LI r6,2
    ST r6,r0,COMMAND
    LI r6,1
    ST r0,r0,X
    LI r2,28
    ST r2,r0,COLOR
    LD r1,r0,LEFT
    LI r2,4
    JAL r7,vertical
    LI r1,15
    ST r1,r0,X
    LI r2,3
    ST r2,r0,COLOR
    LD r1,r0,RIGHT
    LI r2,4
    JAL r7,vertical
    LD r1,r0,BX
    ST r1,r0,X
    LD r1,r0,BY
    ST r1,r0,Y
    LI r1,255
    LD r2,r0,STATE
    ADDI r3,r2,-2
    BNZ r3,try_right_winner
    LI r1,28
try_right_winner:
    ADDI r3,r2,-3
    BNZ r3,ball_color
    LI r1,3
ball_color:
    ST r1,r0,COLOR
    ST r6,r0,COMMAND
    ; Score bars at y=0, from the outside toward the centre.
    ST r0,r0,Y
    LI r2,28
    ST r2,r0,COLOR
    LI r1,0
    LD r2,r0,LSCORE
    LI r3,1
    JAL r7,horizontal
    LI r2,3
    ST r2,r0,COLOR
    LI r1,15
    LD r2,r0,RSCORE
    LI r3,-1
    JAL r7,horizontal
    LI r1,4
    ST r1,r0,COMMAND
    LD r1,r0,FRAME
    ADDI r1,r1,1
    ST r1,r0,FRAME
    LD r1,r0,TIMER
    ST r1,r0,LAST
    JAL r7,status
    JMP main_loop
; r1 Y, r2 low two held bits, r3 scratch. Clamp to [2,12].
paddle:
    ANDI r2,r2,3
    ADDI r3,r2,-1
    BZ r3,paddle_up
    ADDI r3,r2,-2
    BNZ r3,paddle_done
    ADDI r3,r1,-12
    BZ r3,paddle_done
    ADDI r1,r1,1
    JR r7
paddle_up:
    ADDI r3,r1,-2
    BZ r3,paddle_done
    ADDI r1,r1,-1
paddle_done:
    JR r7
; r1 start Y, r2 count, r6=write command. X/color already set.
vertical:
    ST r1,r0,Y
    ST r6,r0,COMMAND
    ADDI r1,r1,1
    ADDI r2,r2,-1
    BNZ r2,vertical
    JR r7
; r1 start X, r2 count (possibly zero), r3 step, r6=write command.
horizontal:
    BZ r2,horizontal_done
    ST r1,r0,X
    ST r6,r0,COMMAND
    ADD r1,r1,r3
    ADDI r2,r2,-1
    JMP horizontal
horizontal_done:
    JR r7
status:
    LD r1,r0,STATE
    LI r2,12
    SHL r1,r1,r2
    LD r2,r0,LSCORE
    LI r3,8
    SHL r2,r2,r3
    OR r1,r1,r2
    LD r2,r0,RSCORE
    OR r1,r1,r2
    ST r1,r0,DEBUG
    JR r7
