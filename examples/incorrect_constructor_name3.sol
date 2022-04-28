pragma solidity ^0.8.11;

contract Missing{
    address payable private owner;

    modifier onlyowner {
        require(msg.sender==owner);
        _;
    }

    function Constructor()
        public
    {
        owner = payable(msg.sender);
    }

    receive () external payable {}

    function withdraw()
        public
        onlyowner
    {
        owner.transfer(payable(this).balance);
    }

}
