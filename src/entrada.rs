//! Leitura da entrada: o laço que imprime o prompt, lê uma linha (do
//! teclado ou de um pipe) e escreve a resposta.
//!
//! Este módulo não sabe nada sobre comandos, armazenamento ou extensões —
//! ele só move texto para dentro e para fora. A lógica de cada linha é
//! delegada ao closure `processar` passado por quem chama `executar`.

use std::io::{self, BufRead, Write};

/// Roda o laço principal do programa.
///
/// Para cada linha lida, chama `processar(linha)`:
/// - `Some(resposta)` → a resposta é impressa e o laço continua.
/// - `None` → sinaliza que o programa deve encerrar (comando `EXIT`).
///
/// O laço também termina sozinho quando a entrada acaba (fim de um pipe),
/// com o mesmo efeito de um `EXIT`.
pub fn executar<F>(mut processar: F)
where
    F: FnMut(&str) -> Option<String>,
{
    let stdin = io::stdin();
    let mut linhas = stdin.lock().lines();

    loop {
        imprimir_prompt();

        let linha = match linhas.next() {
            Some(Ok(linha)) => linha,
            Some(Err(_)) | None => break, // fim da entrada == EXIT
        };

        match processar(&linha) {
            Some(resposta) => println!("{}", resposta),
            None => break, // comando EXIT
        }
    }
}

fn imprimir_prompt() {
    print!("> ");
    // Sem isso o prompt ficaria preso no buffer do stdout até a próxima
    // quebra de linha, e não apareceria antes da resposta em modo interativo.
    let _ = io::stdout().flush();
}
