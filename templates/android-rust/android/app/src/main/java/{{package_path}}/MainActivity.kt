package {{package_name}}

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import {{package_name}}.greet
import {{package_name}}.uniffiEnsureInitialized

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        setContent {
            MaterialTheme {
                Surface(modifier = Modifier.fillMaxSize()) {
                    App()
                }
            }
        }
    }
}

@Composable
fun App() {
    val result = remember {
        androidx.compose.runtime.mutableStateOf("Rust belum dipanggil")
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center
    ) {
        Text(
            text = "Anvil Native + Rust",
            style = MaterialTheme.typography.headlineSmall
        )

        Text(
            text = result.value,
            modifier = Modifier.padding(vertical = 24.dp),
            style = MaterialTheme.typography.bodyLarge
        )

        Button(
            onClick = {
                result.value = runCatching {
                    uniffiEnsureInitialized()
                    greet("Anvil")
                }.fold(
                    onSuccess = { it },
                    onFailure = { error ->
                        "Rust error: ${error.message ?: error.javaClass.simpleName}"
                    }
                )
            }
        ) {
            Text("Call Rust")
        }
    }
}
