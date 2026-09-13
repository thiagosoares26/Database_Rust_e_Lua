-- Extensão proposta: Temperatura
-- Prefixo: temp (chaves no formato temp_*)
--
-- O que ela exercita que CPF e data não exercitam:
--   - CPF valida um dígito verificador e consulta o banco (unicidade).
--   - Data só reformata o texto de entrada, sem transformar o valor.
--   - Esta extensão faz um CÁLCULO (conversão de unidade) e GRAVA no ADD
--     um valor DIFERENTE do que foi digitado: toda temperatura, não
--     importa em que unidade foi informada, é normalizada e armazenada em
--     Celsius. É o único dos três casos em que o valor persistido não é o
--     valor bruto recebido do usuário.
--
-- Formato aceito em ADD: um número (com sinal e/ou casas decimais
-- opcionais) seguido opcionalmente de "C" ou "F" (maiúsculo ou
-- minúsculo). Sem sufixo, assume-se Celsius.
--   Exemplos válidos: "36.5", "36.5C", "97.7F", "-10F", "0"
-- Faixa aceita após a conversão para Celsius: -90 a 60 (limites
-- aproximados já registrados na superfície da Terra).
--
-- GET devolve a temperatura em Celsius e o equivalente em Fahrenheit.

local function eh_numero(s)
    if s == "" then
        return false
    end
    local inicio = 1
    if s:sub(1, 1) == "-" then
        inicio = 2
    end
    if inicio > #s then
        return false
    end
    local viu_ponto = false
    local viu_digito = false
    for pos = inicio, #s do
        local c = s:sub(pos, pos)
        if c == "." and not viu_ponto then
            viu_ponto = true
        elseif c >= "0" and c <= "9" then
            viu_digito = true
        else
            return false
        end
    end
    return viu_digito
end

local function arredondar_uma_casa(x)
    if x >= 0 then
        return math.floor(x * 10 + 0.5) / 10
    end
    return -(math.floor(-x * 10 + 0.5) / 10)
end

-- Retorna a temperatura em Celsius (número) já validada, ou nil + erro.
local function analisar(valor)
    if #valor == 0 then
        return nil, "valor vazio"
    end

    local unidade = valor:sub(-1):upper()
    local parte_numerica = valor
    if unidade == "C" or unidade == "F" then
        parte_numerica = valor:sub(1, #valor - 1)
    else
        unidade = "C"
    end

    if not eh_numero(parte_numerica) then
        return nil, "temperatura deve ser um número, opcionalmente seguido de C ou F (ex.: 36.5C)"
    end

    local numero = tonumber(parte_numerica)
    local celsius = numero
    if unidade == "F" then
        celsius = (numero - 32) * 5 / 9
    end
    celsius = arredondar_uma_casa(celsius)

    if celsius < -90 or celsius > 60 then
        return nil, "temperatura fora da faixa plausível (-90C a 60C)"
    end

    return celsius
end

local function tratar_add(chave, valor)
    local celsius, erro = analisar(valor)
    if not celsius then
        return { ok = false, erro = erro }
    end
    -- Transformação: grava sempre em Celsius, com sufixo "C" — o valor
    -- devolvido ao Rust é diferente do que o usuário digitou quando a
    -- entrada estava em Fahrenheit (ou sem sufixo/arredondada).
    return { ok = true, valor = tostring(celsius) .. "C" }
end

local function tratar_get(chave, valor)
    local celsius, erro = analisar(valor)
    if not celsius then
        return { ok = false, erro = "valor armazenado não é uma temperatura válida" }
    end
    local fahrenheit = arredondar_uma_casa(celsius * 9 / 5 + 32)
    return { ok = true, valor = string.format("%s°C (%s°F)", tostring(celsius), tostring(fahrenheit)) }
end

registrar_extensao("temp", { add = tratar_add, get = tratar_get })
