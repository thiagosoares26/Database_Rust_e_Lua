-- Extensão: Formatador de Data
-- Prefixo: data (chaves no formato data_*)
--
-- ADD: exige o formato ISO 8601 estrito aaaa-mm-dd (quatro dígitos de
--      ano, dois de mês, dois de dia, com zero à esquerda) e que a data
--      exista de verdade no calendário — mês entre 01 e 12, dia dentro do
--      número de dias daquele mês, considerando anos bissextos pela regra
--      completa (divisível por 4, exceto séculos não divisíveis por 400).
-- GET: formata a data armazenada como dd/mm/aaaa.
--
-- Esta extensão NÃO consulta o banco: toda a validação depende só do
-- valor recebido.

local function todo_digito(s)
    if #s == 0 then
        return false
    end
    for i = 1, #s do
        local c = s:sub(i, i)
        if c < "0" or c > "9" then
            return false
        end
    end
    return true
end

local function bissexto(ano)
    if ano % 400 == 0 then
        return true
    end
    if ano % 100 == 0 then
        return false
    end
    return ano % 4 == 0
end

local function dias_no_mes(mes, ano)
    local dias_por_mes = { 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31 }
    if mes == 2 and bissexto(ano) then
        return 29
    end
    return dias_por_mes[mes]
end

-- Retorna uma tabela {ano=, mes=, dia=} (strings, com zero à esquerda
-- preservado) em caso de sucesso, ou nil + mensagem de erro.
local function analisar(valor)
    if #valor ~= 10 or valor:sub(5, 5) ~= "-" or valor:sub(8, 8) ~= "-" then
        return nil, "data deve estar no formato aaaa-mm-dd"
    end

    local s_ano = valor:sub(1, 4)
    local s_mes = valor:sub(6, 7)
    local s_dia = valor:sub(9, 10)

    if not (todo_digito(s_ano) and todo_digito(s_mes) and todo_digito(s_dia)) then
        return nil, "data deve estar no formato aaaa-mm-dd, com apenas dígitos"
    end

    local ano = tonumber(s_ano)
    local mes = tonumber(s_mes)
    local dia = tonumber(s_dia)

    if mes < 1 or mes > 12 then
        return nil, "mês inválido: " .. s_mes
    end

    local maximo = dias_no_mes(mes, ano)
    if dia < 1 or dia > maximo then
        return nil, "dia inválido para o mês informado: " .. s_dia
    end

    return { ano = s_ano, mes = s_mes, dia = s_dia }
end

local function tratar_add(chave, valor)
    local data, erro = analisar(valor)
    if not data then
        return { ok = false, erro = erro }
    end
    return { ok = true, valor = valor }
end

local function tratar_get(chave, valor)
    local data, erro = analisar(valor)
    if not data then
        return { ok = false, erro = "valor armazenado não é uma data válida: " .. erro }
    end
    return { ok = true, valor = data.dia .. "/" .. data.mes .. "/" .. data.ano }
end

registrar_extensao("data", { add = tratar_add, get = tratar_get })
