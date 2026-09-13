//! Interpretação de comandos: transforma uma linha de texto crua em um
//! `Comando` (`ADD`, `GET` ou `EXIT`), ou num erro de sintaxe.
//!
//! Este módulo não sabe nada sobre armazenamento nem sobre extensões —
//! só entende a gramática dos três comandos.

/// Um comando já interpretado, pronto para ser executado pelo motor.
#[derive(Debug, PartialEq, Eq)]
pub enum Comando {
    Add { chave: String, valor: String },
    Get { chave: String },
    Exit,
}

/// Erro de sintaxe ao interpretar uma linha.
#[derive(Debug, PartialEq, Eq)]
pub struct ErroSintaxe(pub String);

/// Interpreta uma linha de entrada. Nunca entra em pânico: qualquer entrada
/// mal formada vira `Err(ErroSintaxe)`, e quem chamar decide como reportar.
pub fn interpretar(linha: &str) -> Result<Comando, ErroSintaxe> {
    let linha = linha.trim_end_matches(['\r', '\n']);

    if linha.trim().is_empty() {
        return Err(ErroSintaxe("linha vazia".to_string()));
    }

    let (verbo, resto) = match linha.find(' ') {
        Some(pos) => (&linha[..pos], &linha[pos + 1..]),
        None => (linha, ""),
    };

    match verbo {
        "ADD" => interpretar_add(resto),
        "GET" => interpretar_get(resto),
        "EXIT" => {
            if resto.trim().is_empty() {
                Ok(Comando::Exit)
            } else {
                Err(ErroSintaxe("EXIT não recebe argumentos".to_string()))
            }
        }
        outro => Err(ErroSintaxe(format!("comando desconhecido: '{}'", outro))),
    }
}

fn interpretar_add(resto: &str) -> Result<Comando, ErroSintaxe> {
    if resto.trim().is_empty() {
        return Err(ErroSintaxe("uso: ADD chave valor".to_string()));
    }
    match resto.find(' ') {
        Some(pos) => {
            let chave = &resto[..pos];
            // O valor é tudo que vem depois do primeiro espaço — pode
            // conter espaços à vontade.
            let valor = &resto[pos + 1..];
            if chave.is_empty() {
                return Err(ErroSintaxe("uso: ADD chave valor".to_string()));
            }
            if valor.is_empty() {
                return Err(ErroSintaxe(
                    "uso: ADD chave valor (valor ausente)".to_string(),
                ));
            }
            Ok(Comando::Add {
                chave: chave.to_string(),
                valor: valor.to_string(),
            })
        }
        None => Err(ErroSintaxe(
            "uso: ADD chave valor (valor ausente)".to_string(),
        )),
    }
}

fn interpretar_get(resto: &str) -> Result<Comando, ErroSintaxe> {
    let chave = resto.trim();
    if chave.is_empty() {
        return Err(ErroSintaxe("uso: GET chave".to_string()));
    }
    if chave.contains(' ') {
        return Err(ErroSintaxe("uso: GET chave (apenas uma chave)".to_string()));
    }
    Ok(Comando::Get {
        chave: chave.to_string(),
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn add_simples() {
        assert_eq!(
            interpretar("ADD cpf_zezinho 12345678909"),
            Ok(Comando::Add {
                chave: "cpf_zezinho".to_string(),
                valor: "12345678909".to_string()
            })
        );
    }

    #[test]
    fn add_valor_com_espacos() {
        assert_eq!(
            interpretar("ADD nome Zezinho da Silva"),
            Ok(Comando::Add {
                chave: "nome".to_string(),
                valor: "Zezinho da Silva".to_string()
            })
        );
    }

    #[test]
    fn get_simples() {
        assert_eq!(
            interpretar("GET cpf_zezinho"),
            Ok(Comando::Get {
                chave: "cpf_zezinho".to_string()
            })
        );
    }

    #[test]
    fn exit_simples() {
        assert_eq!(interpretar("EXIT"), Ok(Comando::Exit));
    }

    #[test]
    fn linha_vazia_e_erro() {
        assert!(interpretar("").is_err());
        assert!(interpretar("   ").is_err());
    }

    #[test]
    fn comando_desconhecido_e_erro() {
        assert!(interpretar("DELETE x").is_err());
    }

    #[test]
    fn add_incompleto_e_erro() {
        assert!(interpretar("ADD").is_err());
        assert!(interpretar("ADD chave").is_err());
    }
}
