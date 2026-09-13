//! Armazenamento chave-valor em memória.
//!
//! Este módulo NÃO sabe nada sobre extensões, Lua ou prefixos de chave.
//! Ele só guarda e recupera pares (chave, valor). A "ponte" (`lua_bridge`)
//! usa uma cópia (`Store` é `Clone`, via `Rc`) para consultar o banco de
//! dentro das funções expostas ao Lua — sem que o armazenamento precise
//! saber que isso está acontecendo.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Armazenamento chave-valor. Barato de clonar: todos os clones apontam
/// para os mesmos dados (via `Rc<RefCell<..>>>`), o que é o que permite a
/// extensão consultar o banco "ao vivo" em vez de receber uma cópia.
#[derive(Clone)]
pub struct Store {
    dados: Rc<RefCell<HashMap<String, String>>>,
}

impl Store {
    pub fn new() -> Self {
        Store {
            dados: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    /// Insere ou substitui o valor associado à chave.
    pub fn inserir(&self, chave: &str, valor: &str) {
        self.dados
            .borrow_mut()
            .insert(chave.to_string(), valor.to_string());
    }

    /// Busca o valor associado a uma chave exata.
    pub fn buscar(&self, chave: &str) -> Option<String> {
        self.dados.borrow().get(chave).cloned()
    }

    /// Retorna todos os pares (chave, valor) cuja chave começa com o
    /// prefixo dado. Não sabe (nem precisa saber) o que o prefixo significa:
    /// é só um filtro textual, usado pela ponte para expor uma consulta
    /// genérica ao Lua.
    pub fn listar_por_prefixo(&self, prefixo: &str) -> Vec<(String, String)> {
        self.dados
            .borrow()
            .iter()
            .filter(|(chave, _)| chave.starts_with(prefixo))
            .map(|(chave, valor)| (chave.clone(), valor.clone()))
            .collect()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn inserir_e_buscar() {
        let store = Store::new();
        store.inserir("a", "1");
        assert_eq!(store.buscar("a"), Some("1".to_string()));
        assert_eq!(store.buscar("b"), None);
    }

    #[test]
    fn clones_compartilham_dados() {
        let store = Store::new();
        let clone = store.clone();
        clone.inserir("x", "y");
        assert_eq!(store.buscar("x"), Some("y".to_string()));
    }

    #[test]
    fn listar_por_prefixo_filtra_corretamente() {
        let store = Store::new();
        store.inserir("cpf_a", "111");
        store.inserir("cpf_b", "222");
        store.inserir("data_c", "333");
        let mut resultado = store.listar_por_prefixo("cpf_");
        resultado.sort();
        assert_eq!(
            resultado,
            vec![
                ("cpf_a".to_string(), "111".to_string()),
                ("cpf_b".to_string(), "222".to_string())
            ]
        );
    }
}
