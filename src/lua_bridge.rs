//! Ponte com o Lua: carrega o diretório `extensions/`, registra as
//! extensões e despacha as chamadas de `ADD`/`GET` para elas.
//!
//! **Este é o único módulo que importa `mlua`.** Nem o armazenamento nem a
//! leitura de entrada sabem que extensões existem — só este módulo.
//!
//! ## Protocolo de registro (resumo — detalhes no README.md)
//!
//! Cada arquivo `.lua` em `extensions/` é executado uma vez, no boot. Para
//! se registrar, ele chama a função global `registrar_extensao(prefixo,
//! tabela)`, onde `tabela` tem os campos opcionais `add` e `get`, cada um
//! uma função Lua `function(chave, valor) -> tabela_resultado`.
//!
//! A extensão nunca recebe uma cópia do banco. Duas funções globais dão
//! acesso "ao vivo" aos dados, e nenhuma delas conhece prefixo algum:
//! - `consultar(chave)` -> valor (string) ou `nil`
//! - `listar(prefixo)` -> lista de `{chave = ..., valor = ...}`
//!
//! `tabela_resultado` é `{ ok = true, valor = "..." }` (valor é opcional —
//! omiti-lo mantém o valor original) ou `{ ok = false, erro = "motivo" }`.

use mlua::{Function, Lua, RegistryKey, Table};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use crate::store::Store;

/// Resultado de repassar um `ADD` ou `GET` para uma extensão.
pub enum ResultadoExtensao {
    /// Nenhuma extensão registrada trata o prefixo desta chave — o motor
    /// deve seguir com o comportamento padrão (gravar/ler o valor cru).
    SemExtensao,
    /// A extensão aceitou a operação. Se houver `Some(valor)`, é o valor
    /// transformado (por exemplo, o valor normalizado a gravar em um ADD,
    /// ou o valor formatado a exibir em um GET); em `None`, usa-se o valor
    /// original.
    Sucesso(Option<String>),
    /// A extensão rejeitou a operação, com o motivo em texto.
    Falha(String),
}

/// Uma extensão registrada: até duas funções Lua (uma por operação).
///
/// Guardamos `RegistryKey` (uma referência sem *lifetime*, própria do Lua
/// registry) em vez de `Function` diretamente, porque `Function<'lua>`
/// carrega o *lifetime* da VM e não poderia ficar guardada dentro da
/// struct `Bridge` junto da própria `Lua`. A função de verdade é
/// recuperada do registry na hora de cada chamada, em `despachar`.
struct RegistroExtensao {
    add: Option<RegistryKey>,
    get: Option<RegistryKey>,
}

/// A ponte em si. Mantém a VM do Lua viva e o registro de extensões.
pub struct Bridge {
    lua: Lua,
    registros: Rc<RefCell<HashMap<String, RegistroExtensao>>>,
}

impl Bridge {
    /// Cria a ponte, expõe as funções globais (`registrar_extensao`,
    /// `consultar`, `listar`) e carrega todo arquivo `.lua` encontrado em
    /// `dir`. Extensões com erro de carregamento são reportadas em stderr
    /// e ignoradas — um `.lua` quebrado não derruba o programa.
    pub fn carregar(store: Store, dir: &Path) -> Bridge {
        let lua = Lua::new();
        let registros: Rc<RefCell<HashMap<String, RegistroExtensao>>> =
            Rc::new(RefCell::new(HashMap::new()));

        expor_globais(&lua, &store, &registros);

        let mut arquivos: Vec<_> = match std::fs::read_dir(dir) {
            Ok(entradas) => entradas
                .filter_map(|entrada| entrada.ok())
                .map(|entrada| entrada.path())
                .filter(|caminho| caminho.extension().map(|ext| ext == "lua").unwrap_or(false))
                .collect(),
            Err(erro) => {
                eprintln!(
                    "Aviso: não foi possível ler o diretório de extensões '{}': {}",
                    dir.display(),
                    erro
                );
                Vec::new()
            }
        };
        // Ordem determinística de carregamento (facilita depuração/testes).
        arquivos.sort();

        for caminho in arquivos {
            if let Err(erro) = carregar_arquivo(&lua, &caminho) {
                eprintln!(
                    "Aviso: falha ao carregar extensão '{}': {}",
                    caminho.display(),
                    erro
                );
            }
        }

        Bridge { lua, registros }
    }

    /// Repassa um `ADD` para a extensão responsável pelo prefixo da chave,
    /// se houver uma.
    pub fn tratar_add(&self, chave: &str, valor: &str) -> ResultadoExtensao {
        self.despachar(chave, valor, |registro| registro.add.as_ref())
    }

    /// Repassa um `GET` para a extensão responsável pelo prefixo da chave,
    /// se houver uma.
    pub fn tratar_get(&self, chave: &str, valor: &str) -> ResultadoExtensao {
        self.despachar(chave, valor, |registro| registro.get.as_ref())
    }

    fn despachar(
        &self,
        chave: &str,
        valor: &str,
        escolher_chave: impl FnOnce(&RegistroExtensao) -> Option<&RegistryKey>,
    ) -> ResultadoExtensao {
        let prefixo = prefixo_da_chave(chave);

        // Importante: o `borrow()` do registro termina *aqui dentro*, antes
        // de chamarmos a função Lua. Assim, se a extensão chamar
        // `consultar`/`listar` (que fazem *outro* empréstimo, desta vez do
        // Store), não há conflito de empréstimo — e mesmo que houvesse,
        // seria com o Store, não com este registro.
        let funcao: Function = {
            let registros = self.registros.borrow();
            let chave_registry = match registros.get(prefixo).and_then(escolher_chave) {
                Some(k) => k,
                None => return ResultadoExtensao::SemExtensao,
            };
            match self.lua.registry_value::<Function>(chave_registry) {
                Ok(f) => f,
                Err(erro) => {
                    return ResultadoExtensao::Falha(format!(
                        "erro interno ao recuperar a extensão: {}",
                        erro
                    ))
                }
            }
        };

        let resultado: mlua::Result<Table> = funcao.call((chave.to_string(), valor.to_string()));
        match resultado {
            Ok(tabela) => interpretar_retorno(tabela),
            Err(erro) => ResultadoExtensao::Falha(formatar_erro_lua(&erro)),
        }
    }
}

/// O prefixo de uma chave é tudo que vem antes do primeiro `_`. Isso é uma
/// regra sintática genérica — nenhum nome de extensão específico aparece
/// aqui, só a convenção de separador.
fn prefixo_da_chave(chave: &str) -> &str {
    match chave.find('_') {
        Some(pos) => &chave[..pos],
        None => chave,
    }
}

fn carregar_arquivo(lua: &Lua, caminho: &Path) -> mlua::Result<()> {
    let codigo = std::fs::read_to_string(caminho)
        .map_err(|erro| mlua::Error::RuntimeError(erro.to_string()))?;
    let nome = caminho.display().to_string();
    lua.load(&codigo).set_name(&nome).exec()
}

/// Expõe ao Lua as três funções globais do protocolo: `registrar_extensao`,
/// `consultar` e `listar`.
fn expor_globais(
    lua: &Lua,
    store: &Store,
    registros: &Rc<RefCell<HashMap<String, RegistroExtensao>>>,
) {
    let globais = lua.globals();

    // registrar_extensao(prefixo, { add = fn, get = fn })
    let registros_para_registro = Rc::clone(registros);
    let registrar = lua
        .create_function(move |lua_ctx, (prefixo, tabela): (String, Table)| {
            let add: Option<RegistryKey> = match tabela.get::<_, Option<Function>>("add")? {
                Some(f) => Some(lua_ctx.create_registry_value(f)?),
                None => None,
            };
            let get: Option<RegistryKey> = match tabela.get::<_, Option<Function>>("get")? {
                Some(f) => Some(lua_ctx.create_registry_value(f)?),
                None => None,
            };
            registros_para_registro
                .borrow_mut()
                .insert(prefixo, RegistroExtensao { add, get });
            Ok(())
        })
        .expect("falha ao criar a função registrar_extensao");
    globais
        .set("registrar_extensao", registrar)
        .expect("falha ao expor registrar_extensao");

    // consultar(chave) -> valor ou nil
    let store_para_consultar = store.clone();
    let consultar = lua
        .create_function(move |_, chave: String| Ok(store_para_consultar.buscar(&chave)))
        .expect("falha ao criar a função consultar");
    globais
        .set("consultar", consultar)
        .expect("falha ao expor consultar");

    // listar(prefixo) -> { {chave=..., valor=...}, ... }
    let store_para_listar = store.clone();
    let listar = lua
        .create_function(move |lua_ctx, prefixo: String| {
            let pares = store_para_listar.listar_por_prefixo(&prefixo);
            let lista = lua_ctx.create_table()?;
            for (indice, (chave, valor)) in pares.into_iter().enumerate() {
                let par = lua_ctx.create_table()?;
                par.set("chave", chave)?;
                par.set("valor", valor)?;
                // Lua é 1-indexado.
                lista.set(indice + 1, par)?;
            }
            Ok(lista)
        })
        .expect("falha ao criar a função listar");
    globais.set("listar", listar).expect("falha ao expor listar");
}

/// Lê a tabela `{ok=.., valor=.., erro=..}` devolvida por uma extensão.
fn interpretar_retorno(tabela: Table) -> ResultadoExtensao {
    let ok: bool = match tabela.get("ok") {
        Ok(v) => v,
        Err(_) => {
            return ResultadoExtensao::Falha(
                "extensão retornou uma tabela sem o campo 'ok' (booleano)".to_string(),
            )
        }
    };

    if ok {
        // `valor` é opcional: se a extensão não devolver nada (ou nil), o
        // motor mantém o valor original.
        let valor: Option<String> = tabela.get::<_, Option<String>>("valor").unwrap_or(None);
        ResultadoExtensao::Sucesso(valor)
    } else {
        let erro: String = tabela
            .get("erro")
            .unwrap_or_else(|_| "erro não especificado pela extensão".to_string());
        ResultadoExtensao::Falha(erro)
    }
}

fn formatar_erro_lua(erro: &mlua::Error) -> String {
    // mlua já inclui uma mensagem legível (inclusive de `error()` do Lua);
    // removemos só o prefixo técnico de "runtime error" quando presente,
    // para a mensagem ficar mais limpa na tela do usuário.
    let texto = erro.to_string();
    texto
        .strip_prefix("runtime error: ")
        .unwrap_or(&texto)
        .to_string()
}
