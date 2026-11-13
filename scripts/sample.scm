;; サンプル: 最寄りの frontier を順に巡って B5F まで降り、魔導書を拾い、
;; 入口まで戻って (exit-dungeon) を呼ぶ。HP が下がったら休む。

(define BOTTOM-FLOOR 5)
(define REST-BELOW 0.55)   ; HP 割合がこれを下回ったら休む
(define REST-UNTIL 0.8)    ; 休むときの目標 HP 割合

;; 各階の到着位置（上り階段）。先頭が現在の階。帰りの経路に使う。
(define arrival-stack '())

(define (px p) (list-ref p 0))
(define (py p) (list-ref p 1))

;; 休んでも回復しなかったときの HP 割合。(party-hp-ratio) は倒れた仲間の
;; 最大 HP も分母に含み、倒れた仲間は復活しないので、割合が閾値まで戻らない。
;; そのため、これ以上は休まない（無駄に休み続けるのを防ぐ）。
(define rest-cap 1.0)

;; 休みすぎないよう最大 4 回まで。
(define (rest-if-hurt)
  (when (and (< (party-hp-ratio) REST-BELOW)
             (< (party-hp-ratio) (- rest-cap 0.001)))
    (let loop ([i 0])
      (let ([before (party-hp-ratio)])
        (when (and (< i 4) (< before REST-UNTIL))
          (rest)
          (if (> (party-hp-ratio) before)
              (loop (+ i 1))
              (set! rest-cap (min rest-cap before))))))))

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

;; 歩行可能で未訪問の既知セル（B5F で床の魔導書を探すのに使う）。
(define (unvisited-cells)
  (let yloop ([y 0] [acc '()])
    (if (>= y 40)
        acc
        (yloop (+ y 1)
               (let xloop ([x 0] [acc acc])
                 (if (>= x 40)
                     acc
                     (xloop (+ x 1)
                            (if (and (member (known-tile x y) '(floor door))
                                     (not (visited? x y)))
                                (cons (list x y) acc)
                                acc))))))))

;; B5F: frontier を尽くし、それでも見つからなければ未訪問セルを順に踏む。
(define (find-book)
  (explore-until (lambda () (has-item? 'magic-book)))
  (let loop ([cells (unvisited-cells)])
    (unless (has-item? 'magic-book)
      (let ([c (nearest (filter (lambda (p) (not (visited? (px p) (py p)))) cells))])
        (when c
          (go-to (px c) (py c))
          (pickup-if-any)
          (loop cells))))))

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
