;; サンプル: 最寄りの frontier を順に巡って B5F まで降り、魔導書を拾い、
;; 入口まで戻って (exit-dungeon) を呼ぶ。HP が下がったら休む。

(define BOTTOM-FLOOR 5)
(define REST-BELOW 0.55)   ; HP 割合がこれを下回ったら休む
(define REST-UNTIL 0.8)    ; 休むときの目標 HP 割合

;; 各階の到着位置（上り階段）。先頭が現在の階。帰りの経路に使う。
(define arrival-stack '())

(define (px p) (list-ref p 0))
(define (py p) (list-ref p 1))

;; 生存メンバーの HP 割合が REST-BELOW を下回ったら REST-UNTIL まで休む（最大 4 回）。
(define (rest-if-hurt)
  (when (< (party-alive-hp-ratio) REST-BELOW)
    (let loop ([i 0])
      (when (and (< i 4) (< (party-alive-hp-ratio) REST-UNTIL))
        (rest)
        (loop (+ i 1))))))

(define (pickup-if-any)
  (when (item-here) (pickup)))

;; (x y) まで既知マップの経路で歩く。到達したら #t、経路がなければ #f。
(define (go-to x y)
  (let loop ()
    (let ([path (path-to x y)])
      (cond [(not path) #f]
            [(null? path) #t]
            [else (follow-path (list (car path))) ; 1 歩ずつ。足元の確認と休憩のため
                  (pickup-if-any)
                  (rest-if-hurt)
                  (loop)]))))

;; cells のうち経路がある最寄りのセルを返す。なければ #f。
(define (nearest cells)
  (let loop ([cs cells] [best #f] [best-len 0])
    (if (null? cs)
        best
        (let ([path (path-to (px (car cs)) (py (car cs)))])
          (if (and path (or (not best) (< (length path) best-len)))
              (loop (cdr cs) (car cs) (length path))
              (loop (cdr cs) best best-len))))))

;; 既知マップから tile を探す。
(define (find-known tile)
  (let yloop ([y 0])
    (if (>= y 40)
        #f
        (let xloop ([x 0])
          (cond [(>= x 40) (yloop (+ y 1))]
                [(equal? (known-tile x y) tile) (list x y)]
                [else (xloop (+ x 1))])))))

;; found? が位置を返すまで frontier を巡る。frontier が尽きたら #f。
(define (explore-until found?)
  (let loop ()
    (let ([hit (found?)])
      (if hit
          hit
          (let ([f (nearest (frontier))])
            (if f
                (begin (go-to (px f) (py f)) (pickup-if-any) (loop))
                #f))))))

;; B5F: frontier を巡り、まだ拾えていなければ既知セル上の魔導書の位置へ直行する。
(define (find-book)
  (explore-until (lambda () (has-item? 'magic-book)))
  (unless (has-item? 'magic-book)
    (let ([items (known-items)])
      (when (pair? items)
        (let ([b (car items)])
          (go-to (px b) (py b))
          (pickup-if-any))))))

(define (dive)
  (set! arrival-stack (cons (position) arrival-stack))
  (when (< (floor-level) BOTTOM-FLOOR)
    (let ([down (explore-until (lambda () (find-known 'stairs-down)))])
      (when down
        (go-to (px down) (py down))
        (when (descend) (dive))))))

(define (return-home)
  (cond [(> (floor-level) 1)
         (let ([up (car arrival-stack)])
           (go-to (px up) (py up))
           (when (ascend)
             (set! arrival-stack (cdr arrival-stack))
             (return-home)))]
        [else
         (let ([e (entrance-pos)])
           (go-to (px e) (py e))
           (exit-dungeon))]))

(dive)
(when (= (floor-level) BOTTOM-FLOOR)
  (find-book)
  (return-home))
