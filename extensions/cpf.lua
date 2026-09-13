-- Extensão: Validador de CPF
-- Prefixo: cpf (chaves no formato cpf_*)
--
-- ADD: valida o dígito verificador (algoritmo oficial da Receita Federal)
--      e garante que o mesmo número de CPF não esteja gravado sob outra
--      chave. A entrada deve conter só os 11 números, sem formatação.
-- GET: formata o valor armazenado como 000.000.000-00.
--
-- Esta é a extensão que CONSULTA O BANCO (via `listar`) para checar
-- unicidade: precisa saber se o CPF já existe em alguma outra chave.

local function somente_digitos(valor)
    if #valor ~= 11 then
        return false
    end
    for i = 1, 11 do
        local c = valor:sub(i, i)
        if c < "0" or c > "9" then
            return false
        end
    end
    return true
end

local function todos_iguais(cpf)
    for i = 2, 11 do
        if cpf:sub(i, i) ~= cpf:sub(1, 1) then
            return false
        end
    end
    return true
end

-- Calcula um dígito verificador do CPF.
-- `tamanho` = 9 para o primeiro dígito (usa os 9 primeiros números,
-- pesos de 10 a 2) e 10 para o segundo (usa os 10 primeiros, incluindo o
-- primeiro dígito verificador, pesos de 11 a 2).
local function digito_verificador(cpf, tamanho)
    local soma = 0
    local peso = tamanho + 1
    for i = 1, tamanho do
        local digito = tonumber(cpf:sub(i, i))
        soma = soma + digito * peso
        peso = peso - 1
    end
    local resto = soma % 11
    if resto < 2 then
        return 0
    end
    return 11 - resto
end

local function cpf_valido(cpf)
    if not somente_digitos(cpf) then
        return false
    end
    -- Sequências de dígito repetido (00000000000, 11111111111, ...) são
    -- rejeitadas por convenção, mesmo quando "passariam" na conta.
    if todos_iguais(cpf) then
        return false
    end
    local d1 = digito_verificador(cpf, 9)
    local d2 = digito_verificador(cpf, 10)
    local esperado = tostring(d1) .. tostring(d2)
    return cpf:sub(10, 11) == esperado
end

local function tratar_add(chave, valor)
    if not somente_digitos(valor) then
        return { ok = false, erro = "CPF deve conter exatamente 11 dígitos numéricos, sem formatação" }
    end
    if not cpf_valido(valor) then
        return { ok = false, erro = "CPF inválido: dígito verificador não confere" }
    end

    -- Unicidade: procura o mesmo número em qualquer outra chave "cpf_*".
    -- Regravar o mesmo CPF na MESMA chave é permitido.
    local existentes = listar("cpf_")
    for _, par in ipairs(existentes) do
        if par.valor == valor and par.chave ~= chave then
            return { ok = false, erro = "CPF já cadastrado na chave '" .. par.chave .. "'" }
        end
    end

    return { ok = true, valor = valor }
end

local function tratar_get(chave, valor)
    if not somente_digitos(valor) then
        return { ok = false, erro = "valor armazenado não é um CPF de 11 dígitos" }
    end
    local formatado = valor:sub(1, 3) .. "." .. valor:sub(4, 6) .. "." ..
        valor:sub(7, 9) .. "-" .. valor:sub(10, 11)
    return { ok = true, valor = formatado }
end

registrar_extensao("cpf", { add = tratar_add, get = tratar_get })
