# Banco de Dados: Rust + Lua

**Integrantes:** Thiago Soares Lima, Tatiana Maturano Zabaneh

Banco de dados chave-valor em memória, no estilo Redis/Memcached, escrito em
Rust, com um sistema de extensões escritas em Lua e embarcadas via `mlua`.

---

## Sumário

- [Como compilar e executar](#como-compilar-e-executar)
- [Uso](#uso)
- [Protocolo de registro de extensões](#protocolo-de-registro-de-extensões)
- [Como acrescentar uma extensão nova](#como-acrescentar-uma-extensão-nova)
- [Estruturas de retorno e tratamento de erro](#estruturas-de-retorno-e-tratamento-de-erro)
- [Consulta ao banco a partir da extensão (e a re-entrância)](#consulta-ao-banco-a-partir-da-extensão-e-a-re-entrância)
- [A extensão proposta: Temperatura](#a-extensão-proposta-temperatura)
- [Módulos do projeto](#módulos-do-projeto)
- [Decisões de projeto](#decisões-de-projeto)
- [Casos de teste](#casos-de-teste)

---

## Como compilar e executar

Pré-requisito: **apenas o Rust instalado** (via `rustup` ou pelo pacote da
distribuição). Não é preciso ter Lua instalado no sistema — a VM do Lua é
compilada a partir do código-fonte pela própria `mlua` (feature
`vendored`), então só falta um compilador C na máquina, que normalmente já
acompanha qualquer instalação de Rust (é o mesmo compilador que o `rustc`
usa para linkar qualquer binário).

```bash
git clone <url-do-repositorio>
cd banco-memoria
cargo build --release
```

Isso baixa as dependências (só a `mlua`, nada além dela) e compila a VM do
Lua embutida junto com o projeto. Rodar:

```bash
# Modo interativo
./target/release/banco-memoria

# Ou, direto pelo cargo (o executável fica em target/release/):
cargo run --release
```

O executável precisa ser rodado com o diretório de trabalho na raiz do
projeto (ou em qualquer lugar que tenha uma pasta `extensions/` ao lado),
já que ele procura extensões em `./extensions/` (caminho relativo). Rodando
com `cargo run` isso já é garantido, porque o cargo executa a partir da
raiz do pacote.

Para rodar os casos de teste (removendo os comentários e passando o
restante como um roteiro de comandos):

```bash
grep -v '^#' casos_teste.txt | grep -v '^$' > /tmp/roteiro.txt
./target/release/banco-memoria < /tmp/roteiro.txt
```

(O `casos_teste.txt` original tem uma linha em branco proposital, no meio
de um bloco comentado, que testa o comportamento do programa com entrada
vazia — o segundo `grep` acima remove *todas* as linhas em branco do
arquivo de teste, inclusive essa; para reproduzir o teste exatamente como
o arquivo descreve, edite manualmente aquele trecho, ou rode o roteiro em
`casos_teste.txt` linha a linha interativamente.)

Para rodar os testes unitários dos módulos `comando` e `store`:

```bash
cargo test
```

## Uso

```
$ ./banco-memoria
> ADD cpf_zezinho 12345678909
OK
> GET cpf_zezinho
123.456.789-09
> GET chave_inexistente
ERRO: chave inexistente
> EXIT
$
```

Os três comandos (`ADD`, `GET`, `EXIT`) e as regras de sintaxe seguem
exatamente o enunciado. Nenhuma entrada — comando desconhecido, linha em
branco, `ADD` incompleto, erro de extensão — derruba o processo; o erro é
reportado com o prefixo `ERRO:` e o prompt volta. O programa também
funciona com um pipe de entrada (`cat roteiro.txt | ./banco-memoria`), e
nesse caso o fim da entrada encerra o programa como um `EXIT`.

---

## Protocolo de registro de extensões

No boot, o motor varre o diretório `extensions/` e executa (uma única vez,
em ordem alfabética do nome do arquivo) todo arquivo `.lua` que encontrar.
Nenhum nome de extensão está fixado no código Rust — quem existe é decidido
inteiramente pelo conteúdo de `extensions/` no momento em que o programa
sobe.

Para se registrar, o script Lua chama, no nível principal do arquivo (fora
de qualquer função), a função global:

```lua
registrar_extensao(prefixo, tabela_de_operacoes)
```

- `prefixo` (string): o texto que antecede o primeiro `_` de uma chave. Uma
  chave `cpf_zezinho` tem prefixo `cpf`; uma chave `data_nascimento` tem
  prefixo `data`; uma chave sem `_` nenhum (`contador`) tem ela mesma como
  prefixo inteiro (e só "bate" com uma extensão registrada exatamente sob
  esse nome — por isso uma chave como `cpfx`, que **começa com** "cpf" mas
  não tem `_`, não é capturada pela extensão de CPF).
- `tabela_de_operacoes`: uma tabela Lua com até dois campos, ambos
  opcionais:
  - `add = function(chave, valor) ... end` — chamada em todo `ADD` cuja
    chave tenha esse prefixo.
  - `get = function(chave, valor) ... end` — chamada em todo `GET` cuja
    chave tenha esse prefixo (`valor` é o que está gravado no banco).

Se só um dos dois campos for definido, a operação não coberta segue o
comportamento padrão (grava/lê o valor cru), como se a extensão não
existisse para aquela operação.

Cada uma dessas funções **devolve uma tabela** com o resultado:

```lua
{ ok = true,  valor = "valor transformado (opcional)" }
{ ok = false, erro  = "mensagem explicando o motivo" }
```

- Em sucesso (`ok = true`), o campo `valor` é opcional. Se presente, é o
  valor que o motor deve usar no lugar do que foi recebido — no `ADD`, é o
  que passa a ser gravado no banco (permite normalizar/transformar o dado,
  não só validá-lo); no `GET`, é o texto formatado que aparece na tela. Se
  `valor` for omitido, o motor usa o valor original.
- Em falha (`ok = false`), `erro` é obrigatório e deve explicar o motivo —
  esse texto é o que aparece depois de `ERRO:` na tela do usuário.

Exemplo mínimo de uma extensão registrada (veja `extensions/cpf.lua`,
`extensions/data.lua` e `extensions/temperatura.lua` para exemplos
completos):

```lua
local function tratar_add(chave, valor)
    if valor == "" then
        return { ok = false, erro = "valor não pode ser vazio" }
    end
    return { ok = true } -- mantém o valor original
end

registrar_extensao("meuprefixo", { add = tratar_add })
```

### Consultar o banco a partir da extensão

Duas funções globais, também expostas pelo motor antes de qualquer arquivo
`.lua` ser carregado, dão à extensão acesso "ao vivo" aos dados — sem
receber uma cópia do banco:

```lua
consultar(chave)   -- devolve o valor (string) da chave, ou nil se não existir
listar(prefixo)    -- devolve uma lista de { chave = ..., valor = ... }
                   -- para toda chave que COMEÇA COM o prefixo dado
```

Nenhuma das duas sabe o que é CPF, data ou qualquer prefixo específico —
`listar` é um filtro textual genérico (`chave:starts_with(prefixo)` do lado
Rust), e quem decide *qual* prefixo consultar é a extensão. É assim que o
validador de CPF verifica unicidade: ele chama `listar("cpf_")` e percorre
o resultado em Lua, procurando o mesmo número em outra chave — o motor em
Rust nunca soube que "cpf_" tem algum significado especial.

---

## Como acrescentar uma extensão nova

Passo a passo para quem nunca mexeu no projeto:

1. Crie um arquivo `minhaextensao.lua` dentro de `extensions/`. O nome do
   arquivo não importa para o motor — só a extensão `.lua`.
2. Escreva uma (ou duas) função Lua no formato `function(chave, valor)`
   que devolve `{ok = true, valor = ...}` ou `{ok = false, erro = ...}`,
   conforme a seção anterior.
3. No fim do arquivo, chame `registrar_extensao("prefixo", { add = ...,
   get = ... })` com o prefixo de chave que a extensão deve tratar.
4. Se a validação precisar checar o que já existe no banco, use
   `consultar(chave)` ou `listar(prefixo)` dentro da função — não é preciso
   (nem possível) receber o banco como argumento.
5. Rode `cargo build` e `./target/release/banco-memoria` de novo. **Não é
   preciso recompilar nada além disso** — o motor descobre o arquivo novo
   sozinho no próximo boot, varrendo `extensions/`.

Não é preciso (nem deve ser preciso) tocar em nenhuma linha de código Rust
para isso funcionar — se for, é sinal de que a extensão está tentando fazer
algo que o protocolo acima não cobre.

---

## Estruturas de retorno e tratamento de erro

Do lado Rust (`src/lua_bridge.rs`), o resultado de repassar um `ADD`/`GET`
para uma extensão é modelado pelo enum:

```rust
pub enum ResultadoExtensao {
    SemExtensao,           // nenhum prefixo registrado bate com a chave
    Sucesso(Option<String>), // ok=true; Some(valor) se houve transformação
    Falha(String),          // ok=false; a mensagem de erro
}
```

O motor (`src/main.rs`) usa esse enum para decidir o que fazer:

- `SemExtensao` em um `ADD` grava o valor cru; em um `GET`, devolve o valor
  cru.
- `Sucesso(Some(valor))` em um `ADD` grava `valor` (não o valor original);
  em um `GET`, exibe `valor` (o texto formatado).
- `Sucesso(None)` usa o valor original em ambos os casos.
- `Falha(motivo)` em qualquer uma das duas operações produz a saída
  `ERRO: {motivo}` na tela, e — no caso do `ADD` — o banco **não** é
  alterado.

Três fontes diferentes de erro chegam a esse `Falha`, todas tratadas do
mesmo jeito por quem chama:

1. **A extensão devolve `{ok=false, erro="..."}` explicitamente** — o texto
   de `erro` vira a mensagem.
2. **A tabela de retorno está malformada** (sem o campo `ok`, por exemplo)
   — `interpretar_retorno` detecta isso e gera uma mensagem própria, para
   que um bug na extensão não derrube o programa.
3. **A função Lua lança um erro em tempo de execução** (`error("...")`,
   índice de tabela inválido, divisão por zero em `nil`, etc.) — o `.call()`
   do `mlua` devolve um `Err`, que é convertido para texto e também vira
   `Falha`.

Em nenhum desses três casos o processo cai: o `main.rs` sempre imprime
`ERRO: <mensagem>` e volta ao prompt.

---

## Consulta ao banco a partir da extensão (e a re-entrância)

Este foi o ponto mais delicado do trabalho: no momento em que uma extensão
chama `consultar`/`listar` durante um `ADD`, o comando `ADD` já está "no
meio" de uma operação sobre esse mesmo banco.

A solução ficou inteiramente no desenho do armazenamento
(`src/store.rs`). `Store` guarda seus dados em `Rc<RefCell<HashMap<String,
String>>>`, e é `Clone` — todo clone aponta para os **mesmos** dados. Isso
por si só não seria suficiente (um `RefCell` emprestado com `borrow_mut()`
entra em pânico se outra chamada tentar emprestá-lo de novo), então a
segunda parte da solução é uma regra que o motor segue à risca em
`main.rs` e em `lua_bridge.rs`:

> **O `Store` nunca fica com um empréstimo mutável (`borrow_mut`) aberto
> enquanto uma função Lua está rodando.**

O fluxo de um `ADD` é:

1. `processar_add` (em `main.rs`) chama `bridge.tratar_add(chave, valor)`.
2. Dentro de `tratar_add` → `despachar` (em `lua_bridge.rs`), a função Lua
   da extensão é chamada. **Nenhum `borrow_mut()` do `Store` acontece
   antes ou durante essa chamada.**
3. Se, durante essa chamada, a extensão invoca `consultar`/`listar`, essas
   funções fazem só `store.borrow()` (empréstimo **imutável**) — o que é
   permitido mesmo que outros empréstimos imutáveis já existam, e não
   conflita com nada, porque não há nenhum empréstimo mutável em aberto
   nesse momento.
4. A função Lua devolve o resultado; `despachar` retorna para
   `processar_add`.
5. **Só então**, de posse do resultado já decidido, `processar_add` chama
   `store.inserir(...)`, que é a única hora em que um `borrow_mut()`
   acontece — e nesse ponto a VM do Lua não está mais rodando, então não
   há chance de reentrância.

Em outras palavras: a "operação em andamento" nunca segura o cadeado do
banco enquanto pergunta algo a si mesma. O empréstimo mutável só existe no
instante isolado do `inserir`, depois que toda a validação (e toda
interação com o Lua) já terminou. Isso também é o que garante a regra de
"regravar o mesmo CPF na mesma chave é permitido": a extensão de CPF
recebe a `chave` como argumento e ignora, na varredura de `listar("cpf_")`,
exatamente essa chave (`par.chave ~= chave`) — o valor antigo ainda está
lá (o `ADD` ainda não escreveu o novo), mas não conta como conflito porque
é a própria chave sendo atualizada.

Um detalhe secundário de implementação: como `mlua::Function` carrega
consigo o *lifetime* da VM do Lua (na versão da `mlua` usada aqui), as
funções registradas por cada extensão não são guardadas diretamente — são
guardadas como `mlua::RegistryKey` (que não tem *lifetime*, podendo viver
dentro da struct `Bridge` ao lado da própria `Lua`) e recuperadas com
`lua.registry_value::<Function>(...)` na hora de cada chamada.

---

## A extensão proposta: Temperatura

Arquivo: `extensions/temperatura.lua`. Prefixo: `temp` (chaves como
`temp_paciente1`).

- **ADD**: aceita um número (com sinal e/ou casas decimais opcionais)
  seguido, opcionalmente, de `C` ou `F` (maiúsculo ou minúsculo) — por
  exemplo `36.5`, `36.5C`, `97.7F`, `-10F`. Sem sufixo, assume-se Celsius.
  O valor é convertido para Celsius, arredondado a uma casa decimal, e
  precisa cair na faixa -90 a 60 (limites aproximados já registrados na
  superfície terrestre) para ser aceito.
- **GET**: devolve a temperatura em Celsius e o equivalente em Fahrenheit,
  por exemplo `36.5°C (97.7°F)`.

Por que essa e não uma variação de CPF/RG/CNPJ: o enunciado already exclui
esse caminho (é o mesmo exercício de dígito verificador com outro peso). A
temperatura foi escolhida porque exercita algo que **nem CPF nem data
exercitam**: uma **transformação de fato** do valor no `ADD`. CPF e data
validam a entrada e gravam o valor exatamente como recebido; a extensão de
temperatura faz um cálculo (conversão de unidade) e **grava algo diferente
do que o usuário digitou** sempre que a entrada não já estava em Celsius —
`ADD temp_x 98.6F` grava `37.0C`, não `98.6F`. É o caso de uso do campo
opcional `valor` em `{ok=true, valor=...}` de um `ADD`, que as duas
extensões obrigatórias nunca usam (elas só usam esse campo no `GET`).

Ela não consulta o banco (assim como a extensão de data) — a validação
depende só do valor recebido.

Casos de teste dela: `casos_teste_temperatura.txt`.

---

## Módulos do projeto

```
src/
├── main.rs        — orquestra os outros quatro módulos; não tem lógica própria
├── comando.rs      — interpreta uma linha de texto em Comando::{Add,Get,Exit} ou erro de sintaxe
├── entrada.rs      — laço de leitura: imprime o prompt, lê uma linha, escreve a resposta
├── store.rs        — armazenamento chave-valor (Rc<RefCell<HashMap<...>>>)
└── lua_bridge.rs    — ÚNICO módulo que importa `mlua`: carrega extensions/, registra e despacha
```

Dependências entre módulos (quem importa quem):

- `main.rs` importa os quatro (`comando`, `entrada`, `lua_bridge`, `store`)
  — é o único lugar que conhece todos eles ao mesmo tempo, e é onde a
  lógica de "o que fazer com o resultado de cada comando" vive.
- `lua_bridge.rs` importa `store.rs` (para poder clonar o `Store` e dar às
  extensões acesso a ele) — mas **não** importa `comando.rs` nem
  `entrada.rs`, e não sabe nada sobre a gramática dos comandos.
- `comando.rs`, `entrada.rs` e `store.rs` não importam uns aos outros nem
  o `lua_bridge` — `comando.rs` só entende texto, `entrada.rs` só move
  texto de entrada/saída (recebe um `closure` genérico de `main.rs`, sem
  saber o que ele faz por dentro), e `store.rs` só guarda pares
  chave-valor, sem saber que existe validação, prefixo ou Lua.
- `mlua` só é importado em `lua_bridge.rs` (checável com
  `grep -rn "use mlua" src/`).

---

## Decisões de projeto

O enunciado deixou várias escolhas em aberto. As que tomamos, e por quê:

- **Prefixo = tudo antes do primeiro `_`.** Escolhemos essa convenção (em
  vez de, por exemplo, um separador configurável) porque é a mais simples
  possível e o próprio enunciado já usa esse padrão nos exemplos (`cpf_*`,
  `data_*`). Chaves sem `_` nenhum têm elas mesmas como prefixo inteiro, o
  que faz com que só batam com uma extensão registrada sob esse nome
  completo — é o que garante que `cpfx` (sem `_`) não seja capturada pela
  extensão de CPF, um dos casos do `casos_teste.txt`.
- **`listar(prefixo)` em vez de expor o banco inteiro, ou uma função
  específica de "existe_duplicado".** Preferimos uma função só, genérica e
  reutilizável por qualquer extensão futura (inclusive a que será usada na
  correção), a uma função batizada para o caso de uso do CPF. O custo é
  que cada extensão que precisa de unicidade faz sua própria varredura em
  Lua — aceitável, dado o tamanho do banco em memória de um projeto como
  este.
- **Arquivos carregados em ordem alfabética.** O enunciado não especifica
  ordem de carregamento; escolhemos alfabética por ser determinística e
  fácil de prever/depurar (duas rodadas do programa com o mesmo diretório
  `extensions/` carregam na mesma ordem).
- **Uma extensão com erro de sintaxe Lua não derruba o boot.** Em vez de
  abortar o programa se um arquivo `.lua` tiver erro, o motor registra um
  aviso em stderr e segue carregando os demais — parece mais alinhado com
  "nenhuma entrada derruba o programa" do que travar o banco inteiro por
  causa de uma extensão quebrada.
- **`mlua` com a feature `vendored`, fixada na versão `0.9.2`.** A feature
  `vendored` compila o Lua a partir do código-fonte embutido na própria
  `mlua`, em vez de depender de uma `liblua` já instalada no sistema — é o
  que permite compilar "numa máquina que só tem Rust instalado", como pede
  o enunciado. A versão foi fixada porque testamos a compilação num
  ambiente com Rust 1.75 (o disponível via `apt` numa imagem Ubuntu
  24.04), e versões mais novas da `mlua` (a partir da que puxa
  `rustc-hash 2.x`) exigem Rust 1.77+; `0.9.2` compila igualmente bem em
  toolchains mais novos, então a fixação não deveria ser um problema em
  outras máquinas, só uma garantia de que builda em qualquer uma.
- **Mensagens de erro em português, livres (não padronizadas por
  código).** O enunciado só exige que a mensagem comece com `ERRO:` e
  explique o motivo — não define um vocabulário fixo, então cada extensão
  (e o motor) escreve a mensagem que achar mais clara para aquele caso.

---

## Casos de teste

- `casos_teste.txt`: o arquivo oficial fornecido com o enunciado (56
  comandos, cumulativos — leia de cima para baixo como uma única sessão).
  Rodado e conferido linha a linha contra a saída real do programa.
- `casos_teste_temperatura.txt`: casos da extensão proposta (Temperatura),
  no mesmo formato.
