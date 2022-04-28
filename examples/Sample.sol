//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;
// TODO make imports work
// import "hardhat/console.sol";


// ----------------------------------------------------------------------------
// Safe maths
// ----------------------------------------------------------------------------
contract SafeMath {
    function safeAdd(uint a, uint b) public pure returns (uint c) {
        c = a + b;
        require(c >= a);
    }
    function safeSub(uint a, uint b) public pure returns (uint c) {
        require(b <= a);
        c = a - b;
    }
    function safeMul(uint a, uint b) public pure returns (uint c) {
        c = a * b;
        require(a == 0 || c / a == b);
    }
    function safeDiv(uint a, uint b) public pure returns (uint c) {
        require(b > 0);
        c = a / b;
    }
}


contract Bank is SafeMath{
    event Received(address, uint);
    event BankTokenPaid(address sender, address recipient, uint amount);
    event BankEthereumPaid(address sender, address recipient, uint amount);
    address public owner; //owner of the contract
    bool public locked;

    mapping(address => uint256) balances;

    // Modifier to ensure the address not zero address
    modifier validAddress(address _addr) {
        require(_addr != address(0), "Not valid address");
        _;
    }

    // Modifier to ensure the caller is the contract owner
    modifier onlyOwner() {
        require(msg.sender == owner, "Not owner");
        _;
    }

    modifier noReentrancy() {
        require(!locked, "No reentrancy");
        locked = true;
        _;
        locked = false;
    }

    function changeOwner(address _newOwner) public onlyOwner validAddress(_newOwner) {
        owner = _newOwner;
    }
    
    constructor(){
        owner = address(msg.sender);
        balances[owner] = 9000000000000000000000000000;
    }

    // Pay ethereum to recipient
    function sendEther(uint256 amount, address recipient) onlyOwner public returns (bool){
        // recipient.transfer(amount);
        // recipient.send(amount);
        (bool success, ) = recipient.call{value: amount}("");
        require(success, "Transfer to recipient failed");
        emit BankEthereumPaid(owner, recipient, amount);
        return true;
    }

    function balanceOf(address account) public view returns (uint256){
        return balances[account];
    }

    // Send token to recipient
    function sendToken(uint256 amount, address recipient) public returns (bool){
        require(balances[msg.sender] > amount, "Sender have insufficient amount to send!");
        balances[msg.sender] = safeSub(balances[msg.sender], amount);
        balances[recipient] = safeAdd(balances[recipient], amount);

        emit BankTokenPaid(msg.sender, recipient, amount);
        return true;
    }

    receive() payable external{
        // console.log("Bank received payment from:", msg.sender);
        emit Received(msg.sender, msg.value);
    }
}


contract Patron {
    uint public count = 0;
    event Received(address, uint);
    
    function getPaid(address payable bankContract, uint amount) external {
        Bank bank = Bank(bankContract);
        bank.sendEther(amount, address(this));
    }

    receive() payable external{
        // console.log("Patron received payment:", msg.value);
        emit Received(msg.sender, msg.value);
    }
}
